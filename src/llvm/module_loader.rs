use crate::front::ast;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::type_helper;
use crate::llvm::builder_helper;
use crate::llvm::compiler::{
    ClosedLabelSetFrame, Compiler, FnTypeInfo, LINUX_STR, OS, StructDef, WINDOWS_STR,
};
use crate::llvm::parser::parse_only;
use crate::llvm::value::build_label_is_error;
use crate::naming;
use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::FunctionValue;
use std::collections::{HashMap, HashSet};

impl<'ctx> Compiler<'ctx> {
    pub fn load_and_compile_module(
        &mut self,
        module_name: &str,
        main_path: Option<&String>,
    ) -> Result<(), SprsError> {
        if self.modules.contains_key(module_name) {
            return Ok(());
        }

        let mut path = format!("{}/{}{}", self.source_path, module_name, naming::SOURCE_EXT);

        if let Some(main_path) = main_path {
            if module_name == "main" {
                path = main_path.clone();
            }
        }

        let source = std::fs::read_to_string(&path).map_err(|load_error| SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 10,
            },
            location: Location::new(path.clone(), Span::DUMMY),
            message: format!("Failed to read module file {}: {}", path, load_error),
            help: None,
        })?;

        self.sources.insert(module_name.to_string(), source.clone());
        self.current_file = path.clone();

        let mut items = parse_only(&source, &path)?;

        self.process_preprocessors(&items);

        let llvm_module_name = items
            .iter()
            .find_map(|item| match item {
                ast::Item::Package(name) => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| module_name.to_string());

        let module = self.context.create_module(&llvm_module_name);

        self.inject_runtime_constants(&module);

        // First, load and compile all imports
        for item in &items {
            if let ast::Item::Import(import_name) = item {
                self.load_and_compile_module(import_name, None)?;
            }
        }

        self.builder.clear_insertion_position();

        let mut known_structs: HashSet<String> = self.struct_defs.keys().cloned().collect();
        for item in &items {
            if let ast::Item::StructItem(struct_item) = item {
                known_structs.insert(struct_item.ident.clone());
            }
        }
        let known_closed_sets: HashSet<String> = self.closed_label_sets.keys().cloned().collect();
        self.apply_function_builds(&mut items, module_name, &path, &mut known_structs)?;
        resolve_item_types(&mut items, &known_structs, &known_closed_sets, &path)?;

        // Declare all function prototypes
        for item in &items {
            match item {
                ast::Item::FunctionItem(func) => {
                    self.declare_fn_prototype(func, &module);
                }
                _ => {}
            }
        }

        let mut private_closed_label_members: Vec<String> = Vec::new();
        let mut private_struct_fields: Vec<String> = Vec::new();
        let mut private_atom_defs: Vec<String> = Vec::new();

        // get enums and structs first
        for item in &items {
            match item {
                ast::Item::StructItem(items) => {
                    for field in &items.fields {
                        if let Some(field_ty) = &field.ty {
                            type_helper::reject_payloadless_label_type(field_ty).map_err(
                                |msg| SprsError::Semantic {
                                    code: ErrorCode {
                                        category: ErrorCategory::Semantic,
                                        number: 11,
                                    },
                                    location: Location::new(String::new(), field.span),
                                    message: msg,
                                    help: None,
                                },
                            )?;
                        }
                    }
                    self.register_struct(items.ident.clone(), items.fields.clone())?;

                    if !items.is_public {
                        for field in &items.fields {
                            let full_name = format!("{}.{}", items.ident, field.ident);
                            private_struct_fields.push(full_name);
                        }
                    }
                }
                ast::Item::ClosedLabelSetItem(set) => {
                    self.register_closed_label_set(set)?;

                    if !set.is_public {
                        for member in &set.members {
                            let full_name = format!("{}.{}", set.ident, member);
                            private_closed_label_members.push(full_name);
                        }
                    }
                }
                ast::Item::AtomItem(def) => {
                    self.register_atom_def(def)?;
                    if !def.is_public {
                        private_atom_defs.push(def.ident.clone());
                    }
                }
                _ => {}
            }
        }

        // Now compile all functions
        for item in &items {
            match item {
                ast::Item::FunctionItem(func) => {
                    self.compile_fn(func, &module)?;
                }
                _ => {}
            }
        }
        if llvm_module_name == "main" {
            if let Some(sprs_main_fn) = module.get_function(naming::INTERNAL_MAIN_FN) {
                let i32_type = self.context.i32_type();
                let main_type = i32_type.fn_type(&[], false);
                let c_main = module.add_function("main", main_type, None);

                let entry = self.context.append_basic_block(c_main, "entry");
                self.builder.position_at_end(entry);

                let main_call = self
                    .builder
                    .build_call(sprs_main_fn, &[], "call_sprs_main")
                    .unwrap();

                // If sprs main returns an error label (`{:error, _}`), panic at the process boundary.
                if let Some(ret_ty) = sprs_main_fn.get_type().get_return_type() {
                    if ret_ty == self.runtime_value_type.into() {
                        let main_ret = match main_call.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(val) => Some(val),
                            inkwell::values::ValueKind::Instruction(_) => None,
                        };
                        if let Some(main_ret) = main_ret {
                            let main_ret_alloca = self
                                .builder
                                .build_alloca(self.runtime_value_type, "main_ret_alloca")
                                .unwrap();
                            self.builder.build_store(main_ret_alloca, main_ret).unwrap();

                            let tag_ptr = self
                                .builder
                                .build_struct_gep(
                                    self.runtime_value_type,
                                    main_ret_alloca,
                                    0,
                                    "main_ret_tag_ptr",
                                )
                                .unwrap();
                            let tag_val = self
                                .builder
                                .build_load(i32_type, tag_ptr, "main_ret_tag")
                                .unwrap()
                                .into_int_value();
                            let data_ptr = self
                                .builder
                                .build_struct_gep(
                                    self.runtime_value_type,
                                    main_ret_alloca,
                                    1,
                                    "main_ret_data_ptr",
                                )
                                .unwrap();
                            let data_val = self
                                .builder
                                .build_load(self.context.i64_type(), data_ptr, "main_ret_data")
                                .unwrap()
                                .into_int_value();

                            // An uncaught `{:error, _}` label returned from
                            // sprs main panics at the process boundary.
                            let is_error = build_label_is_error(self, tag_val, data_val, &module)?;

                            let panic_bb =
                                self.context.append_basic_block(c_main, "main_error_panic");
                            let ok_bb = self.context.append_basic_block(c_main, "main_ok");
                            let _ = self
                                .builder
                                .build_conditional_branch(is_error, panic_bb, ok_bb);

                            self.builder.position_at_end(panic_bb);
                            let panic_msg = self.set_global_constant_str(
                                &module,
                                "Uncaught error in main",
                                true,
                                true,
                            );
                            let panic_ptr = match panic_msg {
                                Some(crate::llvm::compiler::StrConstantResult::Global(
                                    global_value,
                                )) => global_value.as_pointer_value(),
                                Some(crate::llvm::compiler::StrConstantResult::Pointer(
                                    parameter,
                                )) => parameter,
                                None => {
                                    return Err(SprsError::Internal {
                                        message: "Failed to create panic message".to_string(),
                                        location: None,
                                    });
                                }
                            };
                            let panic_fn = self.get_runtime_fn(&module, "__panic")?;
                            self.builder
                                .build_call(panic_fn, &[panic_ptr.into()], "main_panic_call")
                                .unwrap();
                            self.builder.build_unreachable().unwrap();

                            self.builder.position_at_end(ok_bb);
                        }
                    }
                }

                self.builder
                    .build_return(Some(&i32_type.const_int(0, false)))
                    .unwrap();
            }
        }

        self.modules.insert(llvm_module_name, module);

        for private_field in private_struct_fields {
            self.remove_variable(&private_field);
        }

        for private_member in private_closed_label_members {
            self.private_closed_label_members.insert(private_member);
        }

        for private_atom in private_atom_defs {
            self.private_atom_defs.insert(private_atom);
        }

        Ok(())
    }

    fn apply_function_builds(
        &mut self,
        items: &mut Vec<ast::Item>,
        module_name: &str,
        path: &str,
        known_structs: &mut HashSet<String>,
    ) -> Result<(), SprsError> {
        use crate::llvm::function_build::{
            FunctionBuildRegistry, collect_local_function_builds, function_build_source_directive,
            import_public_builds_from_source, insert_builds, known_structs_from_items,
            load_function_build_source, lower_functions_with_builds, resolve_function_build_types,
        };

        let mut registry = FunctionBuildRegistry::default();
        let mut stack = vec![module_name.to_string()];
        let mut known_closed_sets: HashSet<String> = self.closed_label_sets.keys().cloned().collect();
        for item in items.iter() {
            if let ast::Item::ClosedLabelSetItem(set) = item {
                known_closed_sets.insert(set.ident.clone());
            }
        }
        if let Some((source_name, span)) = function_build_source_directive(items, path)? {
            let (mut ext_items, ext_path) = load_function_build_source(
                &source_name,
                span,
                path,
                &self.source_path,
                &mut stack,
            )?;
            let mut ext_known: HashSet<String> = self.struct_defs.keys().cloned().collect();
            ext_known.extend(known_structs_from_items(&ext_items));
            let mut ext_closed: HashSet<String> = self.closed_label_sets.keys().cloned().collect();
            for item in &ext_items {
                if let ast::Item::ClosedLabelSetItem(set) = item {
                    ext_closed.insert(set.ident.clone());
                }
            }
            resolve_item_types(&mut ext_items, &ext_known, &ext_closed, &ext_path)?;
            for item in &ext_items {
                if let ast::Item::StructItem(struct_item) = item {
                    if !self.struct_defs.contains_key(&struct_item.ident) {
                        self.register_struct(
                            struct_item.ident.clone(),
                            struct_item.fields.clone(),
                        )?;
                        known_structs.insert(struct_item.ident.clone());
                    }
                }
            }
            import_public_builds_from_source(&ext_items, &ext_path, &mut registry)?;
        }

        resolve_function_build_types(items, known_structs, &known_closed_sets, path)?;
        let local = collect_local_function_builds(items, path, false)?;
        insert_builds(&mut registry, local)?;
        lower_functions_with_builds(items, &registry, path)?;
        // Expose resolved FunctionBuild contracts for call-site resolution
        // (type parameters + when rules) during prototype declaration.
        for (name, build) in &registry.builds {
            self.function_build_contracts.insert(
                name.clone(),
                (
                    build.signature.type_params.clone(),
                    build.signature.when_rules.clone(),
                ),
            );
        }
        Ok(())
    }

    pub(crate) fn process_preprocessors(&mut self, items: &Vec<ast::Item>) {
        for item in items {
            if let ast::Item::Preprocessor(pre) = item {
                if pre.starts_with("Windows") {
                    self.target_os = OS::Windows;
                } else if pre.starts_with("Linux") {
                    self.target_os = OS::Linux;
                }
            }
        }
    }

    pub(crate) fn register_atom_def(&mut self, def: &ast::AtomDef) -> Result<(), SprsError> {
        if self.atom_defs.contains(&def.ident) {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 4,
                },
                location: self.location(def.span),
                message: format!("Duplicate label: {}", def.ident),
                help: None,
            });
        }
        self.atom_defs.insert(def.ident.clone());
        Ok(())
    }

    pub(crate) fn register_closed_label_set(
        &mut self,
        set: &ast::ClosedLabelSet,
    ) -> Result<(), SprsError> {
        if self.closed_label_sets.contains_key(&set.ident) {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 4,
                },
                location: self.location(set.span),
                message: format!("Duplicate closed label set: {}", set.ident),
                help: None,
            });
        }

        self.closed_label_sets.insert(
            set.ident.clone(),
            ClosedLabelSetFrame {
                members: set.members.clone(),
                is_public: set.is_public,
            },
        );
        Ok(())
    }

    pub(crate) fn inject_runtime_constants(&self, module: &Module<'ctx>) {
        let os_str = match self.target_os {
            OS::Unknown => "Unknown",
            OS::Windows => WINDOWS_STR,
            OS::Linux => LINUX_STR,
        };
        let os_str_val = self.context.const_string(os_str.as_bytes(), true);

        let global = module.add_global(
            os_str_val.get_type(),
            Some(AddressSpace::default()),
            "TARGET_OS",
        );
        global.set_initializer(&os_str_val);
        global.set_linkage(Linkage::Internal);
        global.set_constant(true);
    }

    pub(crate) fn declare_fn_prototype(&mut self, func: &ast::Function, module: &Module<'ctx>) {
        let arg_types: Vec<BasicMetadataTypeEnum> = (0..func.params.len())
            .map(|_| self.context.ptr_type(AddressSpace::default()).into())
            .collect();

        let fn_type = self.runtime_value_type.fn_type(&arg_types, false);
        // Return annotations (`>> T`) describe the success path only.
        // All functions return runtime_value_type so error labels (`{:error, _}`) can propagate
        // across any declared return type (catchable error mechanism).
        let func_name = if func.ident == "main" {
            naming::INTERNAL_MAIN_FN
        } else {
            &func.ident
        };

        let fn_val = if let Some(function_value) = module.get_function(func_name) {
            function_value
        } else {
            module.add_function(func_name, fn_type, None)
        };

        if !func.is_public {
            fn_val.set_linkage(Linkage::Private);
        }

        let (type_params, when_rules) = match &func.build_ref {
            Some(build_name) => self
                .function_build_contracts
                .get(build_name)
                .cloned()
                .unwrap_or_default(),
            None => (Vec::new(), Vec::new()),
        };

        self.fn_types.insert(
            func_name.to_string(),
            FnTypeInfo {
                ret_ty: func.ret_ty.clone(),
                params: func
                    .params
                    .iter()
                    .map(|parameter| parameter.ty.clone())
                    .collect(),
                type_params,
                when_rules,
            },
        );
        // Plain source name also resolves for non-main calls / inference.
        if func.ident == "main" {
            self.fn_types.insert(
                "main".to_string(),
                FnTypeInfo {
                    ret_ty: func.ret_ty.clone(),
                    params: func
                        .params
                        .iter()
                        .map(|parameter| parameter.ty.clone())
                        .collect(),
                    type_params: Vec::new(),
                    when_rules: Vec::new(),
                },
            );
        }
    }
}

fn resolve_item_types(
    items: &mut [ast::Item],
    known_structs: &HashSet<String>,
    known_closed_sets: &HashSet<String>,
    path: &str,
) -> Result<(), SprsError> {
    fn semantic(path: &str, span: Span, message: String) -> SprsError {
        SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 11,
            },
            location: Location::new(path.to_string(), span),
            message,
            help: None,
        }
    }

    for item in items.iter_mut() {
        match item {
            ast::Item::StructItem(struct_item) => {
                for field in &mut struct_item.fields {
                    if let Some(ty) = &mut field.ty {
                        type_helper::resolve_type(
                            ty,
                            known_structs,
                            known_closed_sets,
                            Some(struct_item.ident.as_str()),
                        )
                        .map_err(|message| semantic(path, field.span, message))?;
                    }
                }
            }
            ast::Item::FunctionItem(func) => {
                for param in &mut func.params {
                    if let Some(annot) = &mut param.ty {
                        type_helper::resolve_type(&mut annot.ty, known_structs, known_closed_sets, None)
                            .map_err(|message| semantic(path, param.span, message))?;
                    }
                }
                if let Some(ret_ty) = &mut func.ret_ty {
                    type_helper::resolve_type(ret_ty, known_structs, known_closed_sets, None)
                        .map_err(|message| semantic(path, func.span, message))?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::ast::{Function, Item, Struct};
    use crate::front::type_helper::Type;

    fn collect_known_structs(items: &[Item]) -> HashSet<String> {
        items
            .iter()
            .filter_map(|item| match item {
                Item::StructItem(struct_item) => Some(struct_item.ident.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn resolves_forward_struct_annotations_before_registration() {
        let mut items =
            parse_only("struct A { b >> B } struct B { a >> A }", "forward.sprs").expect("parse");
        let known = collect_known_structs(&items);
        resolve_item_types(&mut items, &known, &HashSet::new(), "forward.sprs").unwrap();

        let Item::StructItem(Struct {
            fields: fields_a, ..
        }) = &items[0]
        else {
            panic!("expected struct A");
        };
        assert_eq!(fields_a[0].ty.as_ref(), Some(&Type::Struct("B".into())));

        let Item::StructItem(Struct {
            fields: fields_b, ..
        }) = &items[1]
        else {
            panic!("expected struct B");
        };
        assert_eq!(fields_b[0].ty.as_ref(), Some(&Type::Struct("A".into())));

        let mut items = parse_only(
            "struct Node { next >> Self, children >> List(Self) }",
            "self_nested.sprs",
        )
        .expect("parse");
        let known = collect_known_structs(&items);
        resolve_item_types(&mut items, &known, &HashSet::new(), "self_nested.sprs").unwrap();
        let Item::StructItem(Struct { fields, .. }) = &items[0] else {
            panic!("expected struct Node");
        };
        assert_eq!(fields[0].ty.as_ref(), Some(&Type::Struct("Node".into())));
        assert_eq!(
            fields[1].ty.as_ref(),
            Some(&Type::App("List".into(), vec![Type::Struct("Node".into())]))
        );
    }

    #[test]
    fn rejects_undefined_named_type_with_location() {
        let mut items =
            parse_only("struct A { value >> DoesNotExist }", "invalid_named.sprs").expect("parse");
        let known = collect_known_structs(&items);
        let err = resolve_item_types(&mut items, &known, &HashSet::new(), "invalid_named.sprs").unwrap_err();
        match err {
            SprsError::Semantic {
                code,
                location,
                message,
                ..
            } => {
                assert_eq!(code.as_string(), "SPRS-SEM-011");
                assert_eq!(message, "Undefined type: DoesNotExist");
                assert_eq!(location.file, "invalid_named.sprs");
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("expected semantic error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_self_outside_struct_with_location() {
        let mut items = parse_only("fn f(x >> Self) {}", "invalid_self.sprs").expect("parse");
        let known = collect_known_structs(&items);
        let err = resolve_item_types(&mut items, &known, &HashSet::new(), "invalid_self.sprs").unwrap_err();
        match err {
            SprsError::Semantic {
                code,
                location,
                message,
                ..
            } => {
                assert_eq!(code.as_string(), "SPRS-SEM-011");
                assert_eq!(
                    message,
                    "`Self` is only valid in struct field type annotations"
                );
                assert_eq!(location.file, "invalid_self.sprs");
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("expected semantic error, got {other:?}"),
        }
        let Item::FunctionItem(Function { params, .. }) = &items[0] else {
            panic!("expected function");
        };
        assert!(params[0].ty.is_some());
    }
}

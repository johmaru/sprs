use crate::front::ast;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::parser::parse_only;
use crate::front::span::Span;
use crate::llvm::compiler::{Compiler, LINUX_STR, OS, WINDOWS_STR};
use crate::llvm::value::build_label_is_error;
use crate::naming;
use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::BasicMetadataTypeEnum;
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
        self.ensure_typed_module(module_name, main_path)?;
        crate::front::type_check::drain_program_function_specializations(
            &mut self.hir_modules,
            &self.function_build_contracts,
        )?;
        let hir_mod =
            self.hir_modules
                .get(module_name)
                .cloned()
                .ok_or_else(|| SprsError::Internal {
                    message: format!("missing typed module {module_name}"),
                    location: None,
                })?;
        for import_name in &hir_mod.imports {
            self.load_and_compile_module(import_name, None)?;
        }
        if self.modules.contains_key(module_name) {
            return Ok(());
        }
        self.current_file = hir_mod.path.clone();
        let llvm_module_name = if hir_mod.is_main {
            "main".to_string()
        } else {
            hir_mod.name.clone()
        };

        let module = self.context.create_module(&llvm_module_name);
        self.apply_module_target(&module);
        self.inject_runtime_constants(&module);
        self.builder.clear_insertion_position();

        for func in &hir_mod.functions {
            self.declare_fn_prototype(func, &module);
        }
        for spec in &hir_mod.function_specializations {
            let mut func = spec.function.clone();
            func.name = self.ensure_function_specialization_name(&spec.id);
            if func.contains_unresolved_type() {
                return Err(SprsError::Internal {
                    message: format!("unresolved type in function specialization {}", func.name),
                    location: None,
                });
            }
            self.declare_fn_prototype(&func, &module);
        }
        let mut private_closed_label_members: Vec<String> = Vec::new();
        let mut private_struct_fields: Vec<String> = Vec::new();
        let mut private_atom_defs: Vec<String> = Vec::new();
        for s in &hir_mod.structs {
            if s.type_params.is_empty() {
                self.register_hir_struct(s)?;
            }
            if !s.is_public {
                for field in &s.fields {
                    private_struct_fields.push(format!("{}.{}", s.name, field.name));
                }
            }
        }
        for specialization in &hir_mod.struct_specializations {
            self.ensure_struct_specialization(specialization)?;
        }
        for set in &hir_mod.closed_label_sets {
            self.register_closed_label_set_hir(set)?;
            if !set.is_public {
                for member in &set.members {
                    private_closed_label_members.push(format!("{}.{}", set.name, member));
                }
            }
        }
        for def in &hir_mod.atoms {
            self.register_atom_def_hir(def)?;
            if !def.is_public {
                private_atom_defs.push(def.name.clone());
            }
        }
        for func in &hir_mod.functions {
            self.compile_fn(func, &module)?;
        }
        for spec in &hir_mod.function_specializations {
            let mut func = spec.function.clone();
            func.name = self.ensure_function_specialization_name(&spec.id);
            self.compile_fn(&func, &module)?;
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
                            let panic_ptr = self
                                .set_global_constant_str(
                                    &module,
                                    "Uncaught error in main",
                                    true,
                                    true,
                                )
                                .as_pointer_value();
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
        use crate::front::function_build::{
            FunctionBuildRegistry, collect_local_function_builds, function_build_source_directive,
            import_public_builds_from_source, insert_builds, known_structs_from_items,
            load_function_build_source, lower_functions_with_builds, resolve_function_build_types,
        };

        let mut registry = FunctionBuildRegistry::default();
        let mut stack = vec![module_name.to_string()];
        let mut known_closed_sets: HashSet<String> =
            self.closed_label_sets.iter().cloned().collect();
        for item in items.iter() {
            if let ast::Item::ClosedLabelSetItem(set) = item {
                known_closed_sets.insert(set.ident.clone());
            }
        }
        if let Some((source_name, span)) = function_build_source_directive(items, path)? {
            let (ext_items, ext_path) = load_function_build_source(
                &source_name,
                span,
                path,
                &self.source_path,
                &mut stack,
            )?;
            let mut ext_known: HashSet<String> = self.struct_defs.keys().cloned().collect();
            ext_known.extend(known_structs_from_items(&ext_items));
            let mut ext_closed: HashSet<String> = self.closed_label_sets.iter().cloned().collect();
            for item in &ext_items {
                if let ast::Item::ClosedLabelSetItem(set) = item {
                    ext_closed.insert(set.ident.clone());
                }
            }
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
                    self.set_compile_target(OS::Windows);
                } else if pre.starts_with("Linux") {
                    self.set_compile_target(OS::Linux);
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
        if self.closed_label_sets.contains(&set.ident) {
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

        self.closed_label_sets.insert(set.ident.clone());
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

    fn ensure_typed_module(
        &mut self,
        module_name: &str,
        main_path: Option<&String>,
    ) -> Result<(), SprsError> {
        if self.hir_modules.contains_key(module_name) {
            return Ok(());
        }
        if self.typecheck_visiting.iter().any(|n| n == module_name) {
            let mut cycle = self.typecheck_visiting.clone();
            cycle.push(module_name.to_string());
            return Err(SprsError::Internal {
                message: format!("circular module import: {}", cycle.join(" -> ")),
                location: None,
            });
        }
        self.typecheck_visiting.push(module_name.to_string());
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
        let imports: Vec<String> = items
            .iter()
            .filter_map(|item| match item {
                ast::Item::Import(name) => Some(name.clone()),
                _ => None,
            })
            .collect();
        for import_name in &imports {
            self.ensure_typed_module(import_name, None)?;
        }
        let mut known_structs: HashSet<String> = self.struct_defs.keys().cloned().collect();
        for item in &items {
            if let ast::Item::StructItem(struct_item) = item {
                known_structs.insert(struct_item.ident.clone());
            }
        }
        self.apply_function_builds(&mut items, module_name, &path, &mut known_structs)?;
        let mut imported = HashMap::new();
        for import_name in &imports {
            if let Some(m) = self.hir_modules.get(import_name) {
                imported.insert(import_name.clone(), m.interface());
            }
        }
        let mut fb_structs = Vec::new();
        for (name, def) in &self.struct_defs {
            fb_structs.push(crate::front::hir::Struct {
                id: crate::front::hir::StructId {
                    module: String::from("%function_build_structs"),
                    name: name.clone(),
                },
                name: name.clone(),
                type_params: Vec::new(),
                fields: def
                    .fields
                    .iter()
                    .map(|f| crate::front::hir::StructField {
                        name: f.ident.clone(),
                        ty: f.ty.clone().unwrap_or(crate::front::type_helper::Type::Any),
                        default_value: None,
                        span: f.span,
                    })
                    .collect(),
                is_public: true,
                span: Span::DUMMY,
            });
        }
        if !fb_structs.is_empty() {
            imported.insert(
                String::from("%function_build_structs"),
                crate::front::hir::ModuleInterface {
                    name: String::from("%function_build_structs"),
                    structs: fb_structs,
                    ..Default::default()
                },
            );
        }
        let typed = crate::front::type_check::check_module(
            &items,
            module_name,
            &path,
            &imported,
            &self.function_build_contracts,
        )?;
        self.hir_modules.insert(module_name.to_string(), typed);
        self.typecheck_visiting.pop();
        Ok(())
    }

    fn register_hir_struct(&mut self, s: &crate::front::hir::Struct) -> Result<(), SprsError> {
        let fields: Vec<ast::StructField> = s
            .fields
            .iter()
            .map(|f| ast::StructField {
                ident: f.name.clone(),
                ty: Some(f.ty.clone()),
                default_value: None,
                span: f.span,
            })
            .collect();
        self.register_struct(s.name.clone(), fields)
    }

    fn register_closed_label_set_hir(
        &mut self,
        set: &crate::front::hir::ClosedLabelSet,
    ) -> Result<(), SprsError> {
        let ast_set = ast::ClosedLabelSet {
            ident: set.name.clone(),
            members: set.members.clone(),
            is_public: set.is_public,
            span: set.span,
        };
        self.register_closed_label_set(&ast_set)
    }

    fn register_atom_def_hir(&mut self, def: &crate::front::hir::AtomDef) -> Result<(), SprsError> {
        let ast_def = ast::AtomDef {
            ident: def.name.clone(),
            is_public: def.is_public,
            span: def.span,
        };
        self.register_atom_def(&ast_def)
    }

    pub(crate) fn declare_fn_prototype(
        &mut self,
        func: &crate::front::hir::Function,
        module: &Module<'ctx>,
    ) {
        let arg_types: Vec<BasicMetadataTypeEnum> = (0..func.params.len())
            .map(|_| self.context.ptr_type(AddressSpace::default()).into())
            .collect();

        let fn_type = self.runtime_value_type.fn_type(&arg_types, false);
        let func_name = if func.name == "main" {
            naming::INTERNAL_MAIN_FN
        } else {
            &func.name
        };

        let fn_val = if let Some(function_value) = module.get_function(func_name) {
            function_value
        } else {
            module.add_function(func_name, fn_type, None)
        };

        if !func.is_public {
            fn_val.set_linkage(Linkage::Private);
        }
    }
}

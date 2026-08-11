use crate::front::ast;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::type_helper;
use crate::front::type_helper::Type;
use crate::llvm::builder_helper;
use crate::llvm::compiler::{Compiler, FnTypeInfo, LINUX_STR, OS, StructDef, Tag, WINDOWS_STR};
use crate::llvm::parser::parse_only;
use crate::llvm::value::build_label_is_error;
use crate::naming;
use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::FunctionValue;
use std::collections::HashMap;

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

        let items = parse_only(&source, &path)?;

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

        // Declare all function prototypes
        for item in &items {
            match item {
                ast::Item::FunctionItem(func) => {
                    self.declare_fn_prototype(func, &module);
                }
                _ => {}
            }
        }

        let mut private_enum_variants: Vec<String> = Vec::new();
        let mut private_struct_fields: Vec<String> = Vec::new();

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
                    self.register_struct(items.ident.clone(), items.fields.clone());

                    if !items.is_public {
                        for field in &items.fields {
                            let full_name = format!("{}.{}", items.ident, field.ident);
                            private_struct_fields.push(full_name);
                        }
                    }
                }
                ast::Item::EnumItem(enm) => {
                    self.register_enum(enm, &module, true);

                    if !enm.is_public {
                        for variant in &enm.variants {
                            let full_name = format!("{}.{}", enm.ident, variant);
                            private_enum_variants.push(full_name);
                        }
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
                                Some(crate::llvm::compiler::StrConstantResult::Global(global_value)) => {
                                    global_value.as_pointer_value()
                                }
                                Some(crate::llvm::compiler::StrConstantResult::Pointer(parameter)) => parameter,
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

        for private_variant in private_enum_variants {
            self.remove_variable(&private_variant);
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

    pub(crate) fn register_enum(
        &mut self,
        enm: &ast::Enum,
        module: &Module<'ctx>,
        is_global: bool,
    ) {
        if enm.variants.is_empty() {
            return;
        }

        self.enum_names.insert(enm.ident.clone());

        // For the runtime EnumInfo struct type : { i8*, i64 }
        let i8_ptr_type = self.context.ptr_type(AddressSpace::default());
        let enum_info_type = self.context.struct_type(
            &[
                i8_ptr_type.into(),             // name
                self.context.i64_type().into(), // variant_index
            ],
            false,
        );

        for (idx, variant) in enm.variants.iter().enumerate() {
            let full_name = format!("{}.{}", enm.ident, variant);

            let enum_tag = self.context.i32_type().const_int(Tag::Enum as u64, false);

            let ptr = if !is_global {
                let current_block = self.builder.get_insert_block().unwrap();
                let function = current_block.get_parent().unwrap();
                let entry_block = function.get_first_basic_block().unwrap();

                if let Some(first_instr) = entry_block.get_first_instruction() {
                    self.builder.position_before(&first_instr)
                } else {
                    self.builder.position_at_end(entry_block)
                };

                let name_ptr = self
                    .builder
                    .build_global_string_ptr(&full_name, &format!("enum_name_{}", full_name))
                    .unwrap();

                let enum_info_ptr = self
                    .builder
                    .build_malloc(enum_info_type, &format!("enum_info_{}", full_name))
                    .unwrap();

                let name_gep = self
                    .builder
                    .build_struct_gep(enum_info_type, enum_info_ptr, 0, "name_ptr")
                    .unwrap();
                self.builder
                    .build_store(name_gep, name_ptr.as_pointer_value())
                    .unwrap();

                let idx_gep = self
                    .builder
                    .build_struct_gep(enum_info_type, enum_info_ptr, 1, "variant_index_ptr")
                    .unwrap();
                let idx_val = self.context.i64_type().const_int(idx as u64, false);
                self.builder.build_store(idx_gep, idx_val).unwrap();

                let enum_info_int = self
                    .builder
                    .build_ptr_to_int(enum_info_ptr, self.context.i64_type(), "enum_info_as_int")
                    .unwrap();

                let alloca = self
                    .builder
                    .build_alloca(self.runtime_value_type, &full_name)
                    .unwrap();

                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, alloca, 0, "enum_tag_ptr")
                    .unwrap();
                self.builder.build_store(tag_ptr, enum_tag).unwrap();

                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, alloca, 1, "enum_data_ptr")
                    .unwrap();
                self.builder.build_store(data_ptr, enum_info_int).unwrap();

                self.builder.position_at_end(current_block);
                alloca
            } else {
                let global_name = format!("enum_name_str_{}", full_name.replace(".", "_"));
                let str_const = self.context.const_string(full_name.as_bytes(), true);
                let global_str = module.add_global(
                    str_const.get_type(),
                    Some(AddressSpace::default()),
                    &global_name,
                );
                global_str.set_initializer(&str_const);
                global_str.set_constant(true);
                global_str.set_linkage(Linkage::Internal);

                let zero = self.context.i32_type().const_int(0, false);
                let name_ptr = unsafe {
                    global_str
                        .as_pointer_value()
                        .const_gep(self.context.i8_type(), &[zero, zero])
                };

                let idx_val = self.context.i64_type().const_int(idx as u64, false);
                let enum_info_const =
                    enum_info_type.const_named_struct(&[name_ptr.into(), idx_val.into()]);

                let global_info_name = format!("enum_info_const_{}", full_name.replace(".", "_"));

                let global_enum_info = module.add_global(
                    enum_info_type,
                    Some(AddressSpace::default()),
                    &global_info_name,
                );
                global_enum_info.set_initializer(&enum_info_const);
                global_enum_info.set_constant(true);
                global_enum_info.set_linkage(Linkage::Internal);

                let enum_info_ptr = global_enum_info.as_pointer_value();
                let enum_info_int = enum_info_ptr.const_to_int(self.context.i64_type());

                let global = module.add_global(
                    self.runtime_value_type,
                    Some(AddressSpace::default()),
                    &full_name,
                );
                let const_val = self
                    .runtime_value_type
                    .const_named_struct(&[enum_tag.into(), enum_info_int.into()]);
                global.set_initializer(&const_val);
                global.set_constant(true);
                global.as_pointer_value()
            };

            self.add_variable(
                full_name.to_string(),
                ptr.into(),
                Type::Enum(enm.ident.clone()),
                false,
                false,
                false,
            );
        }
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

        self.fn_types.insert(
            func_name.to_string(),
            FnTypeInfo {
                ret_ty: func.ret_ty.clone(),
                params: func.params.iter().map(|parameter| parameter.ty.clone()).collect(),
            },
        );
        // Plain source name also resolves for non-main calls / inference.
        if func.ident == "main" {
            self.fn_types.insert(
                "main".to_string(),
                FnTypeInfo {
                    ret_ty: func.ret_ty.clone(),
                    params: func.params.iter().map(|parameter| parameter.ty.clone()).collect(),
                },
            );
        }
    }
}

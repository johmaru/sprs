use crate::front::ast;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::type_helper;
use crate::front::type_helper::Type;
use crate::llvm::builder_helper;
use crate::llvm::parser::parse_only;
use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::FunctionValue;
use std::collections::HashMap;
use crate::llvm::compiler::{Compiler, StructDef, OS, Tag, WINDOWS_STR, LINUX_STR};
use crate::naming;

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

        let source = std::fs::read_to_string(&path).map_err(|e| SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 10,
            },
            location: Location::new(path.clone(), Span::DUMMY),
            message: format!("Failed to read module file {}: {}", path, e),
            help: None,
        })?;

        self.sources.insert(module_name.to_string(), source.clone());

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

                self.builder
                    .build_call(sprs_main_fn, &[], "call_sprs_main")
                    .unwrap();

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

    pub(crate) fn register_enum(&mut self, enm: &ast::Enum, module: &Module<'ctx>, is_global: bool) {
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

            self.add_variable(full_name, ptr.into(), Type::Enum);
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

    pub(crate) fn declare_fn_prototype(&self, func: &ast::Function, module: &Module<'ctx>) {
        let arg_types: Vec<BasicMetadataTypeEnum> = (0..func.params.len())
            .map(|_| self.context.ptr_type(AddressSpace::default()).into())
            .collect();

        let fn_type = if let Some(ret_ty) = &func.ret_ty {
            match ret_ty {
                Type::Any => self.runtime_value_type.fn_type(&arg_types, false),
                Type::Int => self.context.i64_type().fn_type(&arg_types, false),
                Type::Str => self.runtime_value_type.fn_type(&arg_types, false),
                Type::Float => self.context.f64_type().fn_type(&arg_types, false),
                Type::Bool => self.context.bool_type().fn_type(&arg_types, false),
                Type::Unit => self.context.void_type().fn_type(&arg_types, false),
                Type::Enum => self.context.i64_type().fn_type(&arg_types, false),
                Type::Struct(_) => self.runtime_value_type.fn_type(&arg_types, false),

                Type::TypeI8 => self.context.i8_type().fn_type(&arg_types, false),
                Type::TypeU8 => self.context.i8_type().fn_type(&arg_types, false),
                Type::TypeI16 => self.context.i16_type().fn_type(&arg_types, false),
                Type::TypeU16 => self.context.i16_type().fn_type(&arg_types, false),
                Type::TypeI32 => self.context.i32_type().fn_type(&arg_types, false),
                Type::TypeU32 => self.context.i32_type().fn_type(&arg_types, false),
                Type::TypeI64 => self.context.i64_type().fn_type(&arg_types, false),
                Type::TypeU64 => self.context.i64_type().fn_type(&arg_types, false),

                Type::TypeF16 => self.context.f16_type().fn_type(&arg_types, false),
                Type::TypeF32 => self.context.f32_type().fn_type(&arg_types, false),
                Type::TypeF64 => self.context.f64_type().fn_type(&arg_types, false),
            }
        } else {
            self.runtime_value_type.fn_type(&arg_types, false)
        };
        let func_name = if func.ident == "main" {
            naming::INTERNAL_MAIN_FN
        } else {
            &func.ident
        };

        let fn_val = if let Some(f) = module.get_function(func_name) {
            f
        } else {
            module.add_function(func_name, fn_type, None)
        };

        if !func.is_public {
            fn_val.set_linkage(Linkage::Private);
        }
    }
}

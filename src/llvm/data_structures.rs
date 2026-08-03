use inkwell::{
    AddressSpace,
    values::{BasicValueEnum, PointerValue, ValueKind},
};
use crate::{
    front::ast,
    front::error::{SprsError, ErrorCode, ErrorCategory, Location},
    front::span::{Span, Spanned},
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use crate::llvm::value::{create_entry_block_alloca, box_return_value};

pub fn create_list<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    elements: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let list_handle = self_compiler.build_list_from_exprs(elements, module)?;

    let res_ptr = create_entry_block_alloca(self_compiler, "list_res_alloc")?;
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::List as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    // `list_handle` is already an i64 (slab handle) — no ptr_to_int needed.
    self_compiler
        .builder
        .build_store(res_data_ptr, list_handle)
        .unwrap();

    Ok(res_ptr.into())
}

pub fn create_index<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    collection_expr: &Spanned<ast::Expr>,
    index_expr: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let get_fn = self_compiler.get_runtime_fn(module, "__list_get")?;

    let collection_var_ptr = self_compiler
        .compile_expr(collection_expr, module)?
        .into_pointer_value();

    let list_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            collection_var_ptr,
            1,
            "list_data_ptr",
        )
        .unwrap();
    let list_ptr_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            list_data_ptr,
            "list_ptr_int",
        )
        .unwrap()
        .into_int_value();


    let index_val_ptr = self_compiler
        .compile_expr(index_expr, module)?
        .into_pointer_value();

    let index_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            index_val_ptr,
            1,
            "index_data_ptr",
        )
        .unwrap();
    let index_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            index_data_ptr,
            "index_int",
        )
        .unwrap()
        .into_int_value();

    let get_call = self_compiler
        .builder
        .build_call(
            get_fn,
            &[list_ptr_int.into(), index_int.into()],
            "list_get_call",
        )
        .unwrap();

    match get_call.try_as_basic_value() {
        ValueKind::Basic(val) => Ok(val),
        ValueKind::Instruction(_) => Err(SprsError::Internal { message: "Expected basic value from __list_get".to_string(), location: None }),
    }
}

pub fn create_range<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    start_expr: &Spanned<ast::Expr>,
    end_expr: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let range_fn = self_compiler.get_runtime_fn(module, "__range_new")?;
    let start_val_ptr = self_compiler
        .compile_expr(start_expr, module)?
        .into_pointer_value();
    let start_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            start_val_ptr,
            1,
            "start_data_ptr",
        )
        .unwrap();
    let start_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            start_data_ptr,
            "start_int",
        )
        .unwrap()
        .into_int_value();

    let end_val_ptr = self_compiler
        .compile_expr(end_expr, module)?
        .into_pointer_value();
    let end_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            end_val_ptr,
            1,
            "end_data_ptr",
        )
        .unwrap();
    let end_int = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), end_data_ptr, "end_int")
        .unwrap()
        .into_int_value();

    let range_call = self_compiler
        .builder
        .build_call(range_fn, &[start_int.into(), end_int.into()], "range_call")
        .unwrap();
    let range_handle = match range_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal { message: "Expected i64 handle from __range_new".to_string(), location: None });
        }
    };

    let res_ptr = create_entry_block_alloca(self_compiler, "range_res_alloc")?;

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Range as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    // `range_handle` is already an i64 — no ptr_to_int needed.
    self_compiler
        .builder
        .build_store(res_data_ptr, range_handle)
        .unwrap();
    Ok(res_ptr.into())
}

pub fn create_module_access<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    module_name: &str,
    function_name: &str,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let target_module = self_compiler
        .modules
        .get(module_name)
        .ok_or_else(|| SprsError::Semantic { code: ErrorCode { category: ErrorCategory::Semantic, number: 13 }, location: Location::new(String::new(), Span::DUMMY), message: format!("Module '{}' not found", module_name), help: None })?;

    let target_func = target_module.get_function(&function_name).ok_or_else(|| {
        SprsError::Semantic {
            code: ErrorCode { category: ErrorCategory::Semantic, number: 13 },
            location: Location::new(String::new(), Span::DUMMY),
            message: format!(
                "Function '{}' not found in module '{}'",
                function_name, module_name
            ),
            help: None,
        }
    })?;

    let func_in_current_module = if let Some(func) = module.get_function(&function_name) {
        func
    } else {
        module.add_function(&function_name, target_func.get_type(), None)
    };

    let mut compiled_args = Vec::with_capacity(args.len());
    for arg_expr in args {
        let arg_val = self_compiler.compile_expr(arg_expr, module)?.into();
        compiled_args.push(arg_val);
    }

    let call_site = self_compiler
        .builder
        .build_call(func_in_current_module, &compiled_args, "module_func_call")
        .unwrap();

    let return_type_opt = target_func.get_type().get_return_type();
    if return_type_opt.is_none() {
        return create_unit(self_compiler);
    }
    let return_type = return_type_opt.unwrap();

    let result_val = match call_site.try_as_basic_value() {
        ValueKind::Basic(val) => val,
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal { message: "Expected basic value from module function call".to_string(), location: None });
        }
    };

    box_return_value(self_compiler, module, return_type, result_val)
}

pub fn create_field_access<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    struct_expr: &Spanned<ast::Expr>,
    field_index: u32,
    struct_name: &str,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let struct_ptr = self_compiler
        .compile_expr(struct_expr, module)?
        .into_pointer_value();

    let struct_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            struct_ptr,
            1,
            "struct_data_ptr",
        )
        .unwrap();

    let heap_ptr_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            struct_data_ptr,
            "heap_ptr_int",
        )
        .unwrap()
        .into_int_value();

    // `data` is an i64 slab handle — call `__struct_borrow(handle)` to get the
    // raw pointer for field access.
    let struct_borrow_fn = self_compiler.get_runtime_fn(module, "__struct_borrow")?;
    let borrow_call = self_compiler
        .builder
        .build_call(
            struct_borrow_fn,
            &[heap_ptr_int.into()],
            "struct_borrow_call",
        )
        .unwrap();
    let heap_ptr = match borrow_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_pointer_value(),
        _ => return Err(SprsError::Internal { message: "Expected pointer from __struct_borrow".to_string(), location: None }),
    };

    let struct_def = self_compiler
        .struct_defs
        .get(struct_name)
        .ok_or_else(|| SprsError::Semantic { code: ErrorCode { category: ErrorCategory::Semantic, number: 13 }, location: Location::new(String::new(), Span::DUMMY), message: format!("Undefined struct : {}", struct_name), help: None })?;
    let llvm_type = struct_def.llvm_type;
    if field_index as usize >= struct_def.fields.len() {
        return Err(SprsError::Semantic { code: ErrorCode { category: ErrorCategory::Semantic, number: 7 }, location: Location::new(String::new(), Span::DUMMY), message: format!("Field index {} out of bounds for struct '{}' ({} fields)", field_index, struct_name, struct_def.fields.len()), help: None });
    }
    let field_def = &struct_def.fields[field_index as usize];

    let struct_ptr_typed = self_compiler
        .builder
        .build_pointer_cast(
            heap_ptr,
            llvm_type.get_context().ptr_type(AddressSpace::default()),
            "struct_ptr_typed",
        )
        .unwrap();

    let field_ptr = self_compiler
        .builder
        .build_struct_gep(llvm_type, struct_ptr_typed, field_index, "field_ptr")
        .unwrap();

    if let Some(ty) = &field_def.ty {
        match ty {
            crate::front::type_helper::Type::Int
            | crate::front::type_helper::Type::TypeI64
            | crate::front::type_helper::Type::TypeU64 => {
                let val = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), field_ptr, "field_val")
                    .unwrap()
                    .into_int_value();

                let res_ptr =
                    create_entry_block_alloca(self_compiler, "int_field_access_res_alloc")?;
                self_compiler.build_runtime_value_store(
                    res_ptr,
                    StoreTag::Int(Tag::Integer as u64),
                    StoreValue::Int(val),
                    "int_field_access_res",
                );
                return Ok(res_ptr.into());
            }
            crate::front::type_helper::Type::Bool => {
                let val = self_compiler
                    .builder
                    .build_load(
                        self_compiler.context.i64_type(),
                        field_ptr,
                        "bool_field_val",
                    )
                    .unwrap()
                    .into_int_value();

                let res_ptr =
                    create_entry_block_alloca(self_compiler, "bool_field_access_res_alloc")?;
                self_compiler.build_runtime_value_store(
                    res_ptr,
                    StoreTag::Int(Tag::Boolean as u64),
                    StoreValue::Bool(val),
                    "bool_field_access_res",
                );
                return Ok(res_ptr.into());
            }
            crate::front::type_helper::Type::Str => {
                // `data` is an i64 slab handle stored directly in the struct
                // field — load as i64, no pointer conversion.
                let str_handle = self_compiler
                    .builder
                    .build_load(
                        self_compiler.context.i64_type(),
                        field_ptr,
                        "str_field_handle_load",
                    )
                    .unwrap()
                    .into_int_value();
                let res_ptr =
                    create_entry_block_alloca(self_compiler, "str_field_access_res_alloc")?;
                self_compiler.build_runtime_value_store(
                    res_ptr,
                    StoreTag::Int(Tag::String as u64),
                    StoreValue::Int(str_handle),
                    "str_field_access_res",
                );
                return Ok(res_ptr.into());
            }
            _ => { /* Fallback to generic field access */ }
        }
    }

    let field_val = self_compiler
        .builder
        .build_load(self_compiler.runtime_value_type, field_ptr, "field_val")
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "field_access_res_alloc")?;

    self_compiler
        .builder
        .build_store(res_ptr, field_val)
        .unwrap();

    Ok(res_ptr.into())
}

pub fn create_unit<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let res_ptr = create_entry_block_alloca(self_compiler, "unit_res_alloc")?;
    self_compiler.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "unit_res");
    Ok(res_ptr.into())
}

pub fn create_struct_init<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    struct_name: &str,
    field_exprs: &[(String, Spanned<ast::Expr>)],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let struct_def = self_compiler
        .struct_defs
        .get(struct_name)
        .ok_or_else(|| SprsError::Semantic { code: ErrorCode { category: ErrorCategory::Semantic, number: 13 }, location: Location::new(String::new(), Span::DUMMY), message: format!("Undefined struct : {}", struct_name), help: None })?;

    let llvm_type = struct_def.llvm_type;

    // Allocate the struct through the slab runtime so `__drop`/`__clone`
    // recognize it as a slab-owned Struct (not a raw malloc pointer).
    let struct_size = llvm_type
        .size_of()
        .ok_or_else(|| SprsError::Internal { message: "struct type has no size".to_string(), location: None })?;
    let struct_new_fn = self_compiler.get_runtime_fn(module, "__struct_new")?;
    let struct_new_call = self_compiler
        .builder
        .build_call(
            struct_new_fn,
            &[struct_size.into()],
            "struct_new_call",
        )
        .unwrap();
    let struct_handle = match struct_new_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => return Err(SprsError::Internal { message: "Expected i64 handle from __struct_new".to_string(), location: None }),
    };
    let struct_borrow_fn = self_compiler.get_runtime_fn(module, "__struct_borrow")?;
    let borrow_call = self_compiler
        .builder
        .build_call(
            struct_borrow_fn,
            &[struct_handle.into()],
            "struct_borrow_call",
        )
        .unwrap();
    let struct_ptr = match borrow_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_pointer_value(),
        _ => return Err(SprsError::Internal { message: "Expected pointer from __struct_borrow".to_string(), location: None }),
    };
    // `__struct_borrow` returns `*mut u8`; cast to the struct's typed pointer
    // so `build_struct_gep` can index fields.
    let struct_ptr = self_compiler
        .builder
        .build_pointer_cast(
            struct_ptr,
            llvm_type.ptr_type(AddressSpace::default()),
            "struct_ptr_typed",
        )
        .unwrap();

    for (field_name, field_expr) in field_exprs {
        // Re-fetch inside the loop so the immutable borrow ends before
        // `compile_expr`'s mutable borrow (NLL does not end loop-carried
        // borrows between iterations).
        let struct_def = self_compiler
            .struct_defs
            .get(struct_name)
            .expect("struct definition verified at function entry");
        let index = struct_def
            .field_indices
            .get(field_name)
            .copied()
            .ok_or_else(|| {
                SprsError::Semantic {
                    code: ErrorCode { category: ErrorCategory::Semantic, number: 13 },
                    location: Location::new(String::new(), Span::DUMMY),
                    message: format!(
                        "Field '{}' not found in struct '{}'",
                        field_name, struct_name
                    ),
                    help: None,
                }
            })?;

        let field_ty = struct_def
            .fields
            .iter()
            .find(|f| f.ident == *field_name)
            .map(|f| f.ty.clone())
            .ok_or_else(|| {
                SprsError::Semantic {
                    code: ErrorCode { category: ErrorCategory::Semantic, number: 13 },
                    location: Location::new(String::new(), Span::DUMMY),
                    message: format!(
                        "Field definition for '{}' not found in struct '{}'",
                        field_name, struct_name
                    ),
                    help: None,
                }
            })?;

        let value = self_compiler.compile_expr(field_expr, module)?;

        let field_ptr = self_compiler
            .builder
            .build_struct_gep(llvm_type, struct_ptr, index, "field_ptr")
            .map_err(|e| SprsError::Internal { message: e.to_string(), location: None })?;

        if let Some(ty) = &field_ty {
            match ty {
                crate::front::type_helper::Type::Int
                | crate::front::type_helper::Type::TypeI64
                | crate::front::type_helper::Type::TypeU64
                | crate::front::type_helper::Type::Bool => {
                    let val_ptr = value.into_pointer_value();
                    let data_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            val_ptr,
                            1,
                            "int_field_data_ptr",
                        )
                        .unwrap();
                    let int_val = self_compiler
                        .builder
                        .build_load(self_compiler.context.i64_type(), data_ptr, "int_field_val")
                        .unwrap()
                        .into_int_value();
                    self_compiler
                        .builder
                        .build_store(field_ptr, int_val)
                        .unwrap();
                    continue;
                }
                crate::front::type_helper::Type::Str => {
                    let val_ptr = value.into_pointer_value();
                    let data_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            val_ptr,
                            1,
                            "str_field_data_ptr",
                        )
                        .unwrap();
                    // `data` is an i64 slab handle — store it directly in the
                    // struct field (no pointer conversion).
                    let str_handle = self_compiler
                        .builder
                        .build_load(
                            self_compiler.context.i64_type(),
                            data_ptr,
                            "str_field_handle",
                        )
                        .unwrap()
                        .into_int_value();
                    self_compiler
                        .builder
                        .build_store(field_ptr, str_handle)
                        .unwrap();
                    continue;
                }
                _ => { /* Fallback to generic field store */ }
            }
        }

        let val_to_store = if value.is_pointer_value() {
            self_compiler
                .builder
                .build_load(
                    self_compiler.runtime_value_type,
                    value.into_pointer_value(),
                    "field_value",
                )
                .unwrap()
        } else {
            value
        };
        self_compiler
            .builder
            .build_store(field_ptr, val_to_store)
            .unwrap();
    }

    let allloca = self_compiler
        .builder
        .build_alloca(self_compiler.runtime_value_type, "struct_init_res_alloc")
        .unwrap();

    let tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Struct as u64, false);
    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, allloca, 0, "tag_ptr")
        .unwrap();
    self_compiler.builder.build_store(tag_ptr, tag).unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, allloca, 1, "data_ptr")
        .unwrap();
    // Store the slab handle (not the raw pointer) so `__drop`/`__clone`
    // recognize this as a slab-owned Struct.
    self_compiler
        .builder
        .build_store(data_ptr, struct_handle)
        .unwrap();

    Ok(allloca.into())
}

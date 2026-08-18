use crate::llvm::value::{
    box_return_value, create_entry_block_alloca, create_error_label_from_str,
};
use crate::llvm::variable::clone_runtime_value;
use crate::{
    front::ast,
    front::error::{ErrorCategory, ErrorCode, Location, SprsError},
    front::span::{Span, Spanned},
    llvm::builder_helper::{BuilderExt, ContextExt},
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use inkwell::{
    AddressSpace,
    values::{BasicValueEnum, PointerValue, ValueKind},
};
use std::collections::HashMap;

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
    // `buf[i]` / `list[i]`
    //   List   → __list_get
    //   Buffer → __buffer_get
    //   other → Unit sentinel
    let list_get_fn = self_compiler.get_runtime_fn(module, "__list_get")?;
    let buffer_get_fn = self_compiler.get_runtime_fn(module, "__buffer_get")?;

    let collection_var_ptr = self_compiler
        .compile_expr(collection_expr, module)?
        .into_pointer_value();

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            collection_var_ptr,
            0,
            "index_tag_ptr",
        )
        .unwrap();
    let tag_val = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), tag_ptr, "index_tag")
        .unwrap()
        .into_int_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            collection_var_ptr,
            1,
            "index_data_ptr",
        )
        .unwrap();
    let handle_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "index_handle")
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

    let tag_list = self_compiler.get_tag_from_tag_enum(Tag::List);
    let tag_buffer = self_compiler.get_tag_from_tag_enum(Tag::Buffer);
    let is_list = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, tag_val, tag_list, "is_list");
    let is_buffer =
        self_compiler.tag_cmp(inkwell::IntPredicate::EQ, tag_val, tag_buffer, "is_buffer");

    let current_fn = self_compiler.get_current_function();
    let list_bb = self_compiler
        .context
        .append_basic_block(current_fn, "index_list_bb");
    let not_list_bb = self_compiler
        .context
        .append_basic_block(current_fn, "index_not_list_bb");
    let buffer_bb = self_compiler
        .context
        .append_basic_block(current_fn, "index_buffer_bb");
    let other_bb = self_compiler
        .context
        .append_basic_block(current_fn, "index_other_bb");
    let cont_bb = self_compiler
        .context
        .append_basic_block(current_fn, "index_cont_bb");
    let res_ptr = create_entry_block_alloca(self_compiler, "index_res_alloc")?;

    let _ = self_compiler
        .builder
        .build_conditional_branch(is_list, list_bb, not_list_bb);

    self_compiler.builder.position_at_end(not_list_bb);
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_buffer, buffer_bb, other_bb);

    self_compiler.builder.position_at_end(list_bb);
    let list_call = self_compiler
        .builder
        .build_call(
            list_get_fn,
            &[handle_val.into(), index_int.into()],
            "list_get_call",
        )
        .unwrap();
    match list_call.try_as_basic_value() {
        ValueKind::Basic(val) => {
            self_compiler.builder.build_store(res_ptr, val).unwrap();
        }
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "Expected basic value from __list_get".to_string(),
                location: None,
            });
        }
    }
    self_compiler
        .builder
        .build_unconditional_branch(cont_bb)
        .unwrap();

    self_compiler.builder.position_at_end(buffer_bb);
    let buffer_call = self_compiler
        .builder
        .build_call(
            buffer_get_fn,
            &[handle_val.into(), index_int.into()],
            "buffer_get_call",
        )
        .unwrap();
    match buffer_call.try_as_basic_value() {
        ValueKind::Basic(val) => {
            self_compiler.builder.build_store(res_ptr, val).unwrap();
        }
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "Expected basic value from __buffer_get".to_string(),
                location: None,
            });
        }
    }
    self_compiler
        .builder
        .build_unconditional_branch(cont_bb)
        .unwrap();

    self_compiler.builder.position_at_end(other_bb);
    self_compiler.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "unit_sentinel");
    self_compiler
        .builder
        .build_unconditional_branch(cont_bb)
        .unwrap();

    self_compiler.builder.position_at_end(cont_bb);
    Ok(res_ptr.into())
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
            return Err(SprsError::Internal {
                message: "Expected i64 handle from __range_new".to_string(),
                location: None,
            });
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
    let target_module =
        self_compiler
            .modules
            .get(module_name)
            .ok_or_else(|| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), Span::DUMMY),
                message: format!("Module '{}' not found", module_name),
                help: None,
            })?;

    let target_func =
        target_module
            .get_function(&function_name)
            .ok_or_else(|| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), Span::DUMMY),
                message: format!(
                    "Function '{}' not found in module '{}'",
                    function_name, module_name
                ),
                help: None,
            })?;

    let func_in_current_module = if let Some(func) = module.get_function(&function_name) {
        func
    } else {
        module.add_function(&function_name, target_func.get_type(), None)
    };

    self_compiler.check_call_arguments(function_name, args)?;

    let compiled_args = crate::llvm::value::prepare_call_args(self_compiler, args, module)?;

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
            return Err(SprsError::Internal {
                message: "Expected basic value from module function call".to_string(),
                location: None,
            });
        }
    };

    box_return_value(self_compiler, module, return_type, result_val)
}

fn copy_runtime_ptr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    dest: PointerValue<'ctx>,
    src: PointerValue<'ctx>,
    name: &str,
) {
    let val = self_compiler
        .builder
        .build_load(self_compiler.runtime_value_type, src, name)
        .unwrap();
    self_compiler.builder.build_store(dest, val).unwrap();
}

fn ptr_is_null<'ctx>(
    self_compiler: &Compiler<'ctx>,
    ptr: PointerValue<'ctx>,
    name: &str,
) -> inkwell::values::IntValue<'ctx> {
    let addr = self_compiler
        .builder
        .build_ptr_to_int(
            ptr,
            self_compiler.context.i64_type(),
            &format!("{name}_addr"),
        )
        .unwrap();
    self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            addr,
            self_compiler.context.i64_type().const_zero(),
            &format!("{name}_is_null"),
        )
        .unwrap()
}

fn emit_struct_track<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    module: &inkwell::module::Module<'ctx>,
    struct_handle: inkwell::values::IntValue<'ctx>,
    field_ptr: PointerValue<'ctx>,
    tag: inkwell::values::IntValue<'ctx>,
    data: inkwell::values::IntValue<'ctx>,
    data_only: bool,
) -> Result<inkwell::values::IntValue<'ctx>, SprsError> {
    let track_fn = self_compiler.get_runtime_fn(module, "__struct_track_value")?;
    let field_i8 = self_compiler
        .builder
        .build_pointer_cast(
            field_ptr,
            self_compiler.context.ptr_type(AddressSpace::default()),
            "struct_field_i8",
        )
        .unwrap();
    let data_only_val = self_compiler
        .context
        .i32_type()
        .const_int(if data_only { 1 } else { 0 }, false);
    let call = self_compiler
        .builder
        .build_call(
            track_fn,
            &[
                struct_handle.into(),
                field_i8.into(),
                tag.into(),
                data.into(),
                data_only_val.into(),
            ],
            "struct_track_call",
        )
        .unwrap();
    match call.try_as_basic_value() {
        ValueKind::Basic(val) => Ok(val.into_int_value()),
        _ => Err(SprsError::Internal {
            message: "Expected i32 from __struct_track_value".to_string(),
            location: None,
        }),
    }
}

pub fn create_field_access<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    struct_expr: &Spanned<ast::Expr>,
    field_index: u32,
    struct_name: &str,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let result_ptr = create_entry_block_alloca(self_compiler, "field_access_result")?;
    let parent_fn = self_compiler.get_current_function();
    let invalid_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "field_access_invalid");
    let body_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "field_access_body");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "field_access_merge");

    let struct_ptr = self_compiler
        .compile_expr(struct_expr, module)?
        .into_pointer_value();

    let struct_tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            struct_ptr,
            0,
            "struct_tag_ptr",
        )
        .unwrap();
    let struct_tag = self_compiler
        .builder
        .build_load(
            self_compiler.context.i32_type(),
            struct_tag_ptr,
            "struct_tag",
        )
        .unwrap()
        .into_int_value();
    let is_struct = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            struct_tag,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Struct as u64, false),
            "is_struct_tag",
        )
        .unwrap();
    let tag_ok_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "field_access_tag_ok");
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_struct, tag_ok_bb, invalid_bb);
    self_compiler.builder.position_at_end(tag_ok_bb);

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
        _ => {
            return Err(SprsError::Internal {
                message: "Expected pointer from __struct_borrow".to_string(),
                location: None,
            });
        }
    };

    let is_null = ptr_is_null(self_compiler, heap_ptr, "struct_borrow");
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_null, invalid_bb, body_bb);

    self_compiler.builder.position_at_end(invalid_bb);
    let err_ptr = create_error_label_from_str(self_compiler, "Invalid struct handle", module)?;
    copy_runtime_ptr(self_compiler, result_ptr, err_ptr, "invalid_handle_err");
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(body_bb);

    let struct_def =
        self_compiler
            .struct_defs
            .get(struct_name)
            .ok_or_else(|| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), Span::DUMMY),
                message: format!("Undefined struct : {}", struct_name),
                help: None,
            })?;
    let llvm_type = struct_def.llvm_type;
    if field_index as usize >= struct_def.fields.len() {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 7,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: format!(
                "Field index {} out of bounds for struct '{}' ({} fields)",
                field_index,
                struct_name,
                struct_def.fields.len()
            ),
            help: None,
        });
    }
    let field_def_ty = struct_def.fields[field_index as usize].ty.clone();

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

    if let Some(ty) = &field_def_ty {
        match ty {
            crate::front::type_helper::Type::Int
            | crate::front::type_helper::Type::TypeI64
            | crate::front::type_helper::Type::TypeU64 => {
                let val = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), field_ptr, "field_val")
                    .unwrap()
                    .into_int_value();
                self_compiler.build_runtime_value_store(
                    result_ptr,
                    StoreTag::Int(Tag::Integer as u64),
                    StoreValue::Int(val),
                    "int_field_access_res",
                );
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
                self_compiler.build_runtime_value_store(
                    result_ptr,
                    StoreTag::Int(Tag::Boolean as u64),
                    StoreValue::Bool(val),
                    "bool_field_access_res",
                );
            }
            crate::front::type_helper::Type::Float | crate::front::type_helper::Type::TypeF64 => {
                let val = self_compiler
                    .builder
                    .build_load(
                        self_compiler.context.i64_type(),
                        field_ptr,
                        "float_field_val",
                    )
                    .unwrap()
                    .into_int_value();
                self_compiler.build_runtime_value_store(
                    result_ptr,
                    StoreTag::Int(Tag::Float as u64),
                    StoreValue::Int(val),
                    "float_field_access_res",
                );
            }
            crate::front::type_helper::Type::Str => {
                let str_handle = self_compiler
                    .builder
                    .build_load(
                        self_compiler.context.i64_type(),
                        field_ptr,
                        "str_field_handle_load",
                    )
                    .unwrap()
                    .into_int_value();
                self_compiler.build_runtime_value_store(
                    result_ptr,
                    StoreTag::Int(Tag::String as u64),
                    StoreValue::Int(str_handle),
                    "str_field_access_res",
                );
                let cloned = clone_runtime_value(self_compiler, result_ptr, module)?;
                copy_runtime_ptr(self_compiler, result_ptr, cloned, "str_field_cloned");
            }
            _ => {
                let field_val = self_compiler
                    .builder
                    .build_load(self_compiler.runtime_value_type, field_ptr, "field_val")
                    .unwrap();
                self_compiler
                    .builder
                    .build_store(result_ptr, field_val)
                    .unwrap();
                let cloned = clone_runtime_value(self_compiler, result_ptr, module)?;
                copy_runtime_ptr(self_compiler, result_ptr, cloned, "generic_field_cloned");
            }
        }
    } else {
        let field_val = self_compiler
            .builder
            .build_load(self_compiler.runtime_value_type, field_ptr, "field_val")
            .unwrap();
        self_compiler
            .builder
            .build_store(result_ptr, field_val)
            .unwrap();
        let cloned = clone_runtime_value(self_compiler, result_ptr, module)?;
        copy_runtime_ptr(self_compiler, result_ptr, cloned, "untyped_field_cloned");
    }

    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    self_compiler.builder.position_at_end(merge_bb);
    Ok(result_ptr.into())
}

pub fn create_struct_init<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    struct_name: &str,
    field_exprs: &[(String, Spanned<ast::Expr>)],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    // ---- field validation before any LLVM emission ----
    let struct_def =
        self_compiler
            .struct_defs
            .get(struct_name)
            .ok_or_else(|| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), Span::DUMMY),
                message: format!("Undefined struct : {}", struct_name),
                help: None,
            })?;

    for (field_name, field_expr) in field_exprs {
        if !struct_def.field_indices.contains_key(field_name) {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), field_expr.span),
                message: format!(
                    "unknown field `{}` in init {}",
                    field_name, struct_name
                ),
                help: Some("fields must match the struct declaration".to_string()),
            });
        }
    }
    for (idx, (field_name, field_expr)) in field_exprs.iter().enumerate() {
        if field_exprs[..idx]
            .iter()
            .any(|(previous, _)| previous == field_name)
        {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), field_expr.span),
                message: format!(
                    "duplicate field `{}` in init {}",
                    field_name, struct_name
                ),
                help: Some("each field may be initialized at most once".to_string()),
            });
        }
    }
    for field in &struct_def.fields {
        let has_explicit = field_exprs.iter().any(|(name, _)| name == &field.ident);
        if !has_explicit && field.default_value.is_none() {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), field.span),
                message: format!(
                    "missing required field `{}` in init {}",
                    field.ident, struct_name
                ),
                help: Some(
                    "provide a value or add a default to the field declaration".to_string(),
                ),
            });
        }
    }

    // Fields in declaration order: explicit value wins, otherwise the
    // field's `default_value` is evaluated at the init site. The struct
    // definition data is cloned so later mutable compiler use does not
    // conflict with the borrow.
    let fields: Vec<crate::front::ast::StructField> = struct_def.fields.clone();
    let field_indices: HashMap<String, u32> = struct_def.field_indices.clone();
    let llvm_type = struct_def.llvm_type;
    let ordered_fields: Vec<(String, &Spanned<ast::Expr>, u32)> = fields
        .iter()
        .map(|field| {
            let index = field_indices[&field.ident];
            match field_exprs.iter().find(|(name, _)| name == &field.ident) {
                Some((name, expr)) => (name.clone(), expr, index),
                None => (
                    field.ident.clone(),
                    field.default_value.as_ref().expect("validated above"),
                    index,
                ),
            }
        })
        .collect();

    let result_ptr = create_entry_block_alloca(self_compiler, "struct_init_result")?;
    let parent_fn = self_compiler.get_current_function();
    let invalid_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "struct_init_invalid");
    let body_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "struct_init_body");
    let field_err_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "struct_init_field_err");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "struct_init_merge");

    let struct_size = llvm_type.size_of().ok_or_else(|| SprsError::Internal {
        message: "struct type has no size".to_string(),
        location: None,
    })?;
    let struct_new_fn = self_compiler.get_runtime_fn(module, "__struct_new")?;
    let struct_new_call = self_compiler
        .builder
        .build_call(struct_new_fn, &[struct_size.into()], "struct_new_call")
        .unwrap();
    let struct_handle = match struct_new_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => {
            return Err(SprsError::Internal {
                message: "Expected i64 handle from __struct_new".to_string(),
                location: None,
            });
        }
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
        _ => {
            return Err(SprsError::Internal {
                message: "Expected pointer from __struct_borrow".to_string(),
                location: None,
            });
        }
    };

    let is_null = ptr_is_null(self_compiler, struct_ptr, "struct_init_borrow");
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_null, invalid_bb, body_bb);

    self_compiler.builder.position_at_end(invalid_bb);
    let err_ptr = create_error_label_from_str(self_compiler, "Invalid struct handle", module)?;
    copy_runtime_ptr(self_compiler, result_ptr, err_ptr, "init_invalid_handle");
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(body_bb);
    let struct_ptr = self_compiler
        .builder
        .build_pointer_cast(
            struct_ptr,
            self_compiler.context.ptr_type(AddressSpace::default()),
            "struct_ptr_typed",
        )
        .unwrap();

    for (field_name, field_expr, index) in &ordered_fields {
        let field_ty = fields
            .iter()
            .find(|f| &f.ident == field_name)
            .and_then(|f| f.ty.clone());

        let value = self_compiler.compile_owned_expr(field_expr, module, "struct_field_owned")?;

        let field_ptr = self_compiler
            .builder
            .build_struct_gep(llvm_type, struct_ptr, *index, "field_ptr")
            .map_err(|e| SprsError::Internal {
                message: e.to_string(),
                location: None,
            })?;

        let mut track: Option<(inkwell::values::IntValue, inkwell::values::IntValue, bool)> = None;
        if let Some(ty) = &field_ty {
            match ty {
                crate::front::type_helper::Type::Int
                | crate::front::type_helper::Type::TypeI64
                | crate::front::type_helper::Type::TypeU64
                | crate::front::type_helper::Type::Bool
                | crate::front::type_helper::Type::Float
                | crate::front::type_helper::Type::TypeF64 => {
                    let data_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            value,
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
                }
                crate::front::type_helper::Type::Str => {
                    let data_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            value,
                            1,
                            "str_field_data_ptr",
                        )
                        .unwrap();
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
                    let tag = self_compiler
                        .context
                        .i32_type()
                        .const_int(Tag::String as u64, false);
                    track = Some((tag, str_handle, true));
                }
                _ => {
                    let val_to_store = self_compiler
                        .builder
                        .build_load(self_compiler.runtime_value_type, value, "field_value")
                        .unwrap();
                    self_compiler
                        .builder
                        .build_store(field_ptr, val_to_store)
                        .unwrap();
                    let tag_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            value,
                            0,
                            "generic_field_tag_ptr",
                        )
                        .unwrap();
                    let tag = self_compiler
                        .builder
                        .build_load(
                            self_compiler.context.i32_type(),
                            tag_ptr,
                            "generic_field_tag",
                        )
                        .unwrap()
                        .into_int_value();
                    let data_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            value,
                            1,
                            "generic_field_data_ptr",
                        )
                        .unwrap();
                    let data = self_compiler
                        .builder
                        .build_load(
                            self_compiler.context.i64_type(),
                            data_ptr,
                            "generic_field_data",
                        )
                        .unwrap()
                        .into_int_value();
                    track = Some((tag, data, false));
                }
            }
        } else {
            let val_to_store = self_compiler
                .builder
                .build_load(self_compiler.runtime_value_type, value, "field_value")
                .unwrap();
            self_compiler
                .builder
                .build_store(field_ptr, val_to_store)
                .unwrap();
            let tag_ptr = self_compiler
                .builder
                .build_struct_gep(
                    self_compiler.runtime_value_type,
                    value,
                    0,
                    "untyped_field_tag_ptr",
                )
                .unwrap();
            let tag = self_compiler
                .builder
                .build_load(
                    self_compiler.context.i32_type(),
                    tag_ptr,
                    "untyped_field_tag",
                )
                .unwrap()
                .into_int_value();
            let data_ptr = self_compiler
                .builder
                .build_struct_gep(
                    self_compiler.runtime_value_type,
                    value,
                    1,
                    "untyped_field_data_ptr",
                )
                .unwrap();
            let data = self_compiler
                .builder
                .build_load(
                    self_compiler.context.i64_type(),
                    data_ptr,
                    "untyped_field_data",
                )
                .unwrap()
                .into_int_value();
            track = Some((tag, data, false));
        }

        if let Some((tag, data, data_only)) = track {
            let ok = emit_struct_track(
                self_compiler,
                module,
                struct_handle,
                field_ptr,
                tag,
                data,
                data_only,
            )?;
            let failed = self_compiler
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    ok,
                    self_compiler.context.i32_type().const_zero(),
                    "struct_track_failed",
                )
                .unwrap();
            let cont_bb = self_compiler
                .context
                .append_basic_block(parent_fn, "struct_field_track_ok");
            let _ = self_compiler
                .builder
                .build_conditional_branch(failed, field_err_bb, cont_bb);
            self_compiler.builder.position_at_end(cont_bb);
        }
    }

    self_compiler.build_runtime_value_store(
        result_ptr,
        StoreTag::Int(Tag::Struct as u64),
        StoreValue::Int(struct_handle),
        "struct_init_res",
    );
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(field_err_bb);
    let drop_fn = self_compiler.get_runtime_fn(module, "__drop")?;
    let struct_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Struct as u64, false);
    let _ = self_compiler.builder.build_call(
        drop_fn,
        &[struct_tag.into(), struct_handle.into()],
        "drop_invalid_struct_field",
    );
    let err_ptr =
        create_error_label_from_str(self_compiler, "Invalid struct field storage", module)?;
    copy_runtime_ptr(self_compiler, result_ptr, err_ptr, "init_field_err");
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(merge_bb);
    Ok(result_ptr.into())
}

pub fn create_unit<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let res_ptr = create_entry_block_alloca(self_compiler, "unit_res_alloc")?;
    self_compiler.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "unit_res");
    Ok(res_ptr.into())
}

use crate::front::label_name::LabelName;
use crate::llvm::data_structures::create_unit;
use crate::llvm::value::{
    build_dynamic_string, build_label_is_error, create_entry_block_alloca,
    create_error_label_from_str, create_label,
};
use crate::llvm::variable::{clone_runtime_value, move_variable, var_load_at_init_variable};
use crate::{
    front::ast,
    front::error::{ErrorCategory, ErrorCode, Location, SprsError},
    front::span::{Span, Spanned},
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use inkwell::{
    AddressSpace,
    values::{BasicValueEnum, PointerValue, ValueKind},
};

pub fn call_builtin_macro_println<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let print_fn = self_compiler.get_runtime_fn(module, "__println")?;

    let list_ptr = self_compiler.build_list_from_exprs(args, module)?;

    self_compiler
        .builder
        .build_call(print_fn, &[list_ptr.into()], "println_call")
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "println_res_alloc")?;
    self_compiler.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "unit_res");

    return Ok(res_ptr.into());
}

pub fn call_builtin_macro_list_push<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "list_push expects 2 arguments".to_string(),
            help: None,
        });
    }
    let list_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let compiled_val_ptr = self_compiler
        .compile_expr(&args[1], module)?
        .into_pointer_value();
    let (val_ptr, source_var) = if let ast::Expr::Var(name) = &args[1].node {
        if let Some(src) = self_compiler.get_variables(name) {
            if src.always_clone {
                (
                    clone_runtime_value(self_compiler, src.value.into_pointer_value(), module)?,
                    None,
                )
            } else {
                (compiled_val_ptr, Some((src.value, name)))
            }
        } else {
            (compiled_val_ptr, None)
        }
    } else {
        (compiled_val_ptr, None)
    };

    let list_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            list_ptr,
            1,
            "list_data_ptr",
        )
        .unwrap();
    let list_vec_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            list_data_ptr,
            "list_vec_int",
        )
        .unwrap()
        .into_int_value();
    // `list_vec_int` is already an i64 slab handle — `__list_push` takes it
    // directly, no pointer conversion needed.
    let list_handle = list_vec_int;

    let target_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 0, "val_tag_ptr")
        .unwrap();
    let val_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), target_ptr, "val_tag")
        .unwrap()
        .into_int_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 1, "val_data_ptr")
        .unwrap();
    let val_data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "val_data")
        .unwrap()
        .into_int_value();

    let list_push_fn = self_compiler.get_runtime_fn(module, "__list_push")?;
    self_compiler
        .builder
        .build_call(
            list_push_fn,
            &[list_handle.into(), val_tag.into(), val_data.into()],
            "list_push_call",
        )
        .unwrap();

    if let Some((source_ptr, source_name)) = source_var {
        move_variable(self_compiler, &source_ptr, source_name);
    }

    let res_ptr = create_entry_block_alloca(self_compiler, "list_push_res_alloc")?;
    self_compiler.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "unit_res");

    return Ok(res_ptr.into());
}

/// @bufLen(buf) — length of a Buffer as Integer (0 for stale / non-Buffer).
pub fn call_builtin_macro_buf_len<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "bufLen expects 1 argument".to_string(),
            help: None,
        });
    }
    let buf_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let buf_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, buf_ptr, 1, "buf_data_ptr")
        .unwrap();
    let buf_handle = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), buf_data_ptr, "buf_handle")
        .unwrap()
        .into_int_value();

    let len_fn = self_compiler.get_runtime_fn(module, "__buffer_len")?;
    let len_val = match self_compiler
        .builder
        .build_call(len_fn, &[buf_handle.into()], "buffer_len_call")
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__buffer_len returned void".to_string(),
                location: None,
            });
        }
    };

    let res_ptr = create_entry_block_alloca(self_compiler, "buf_len_res_alloc")?;
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
                .const_int(Tag::Integer as u64, false),
        )
        .unwrap();
    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, len_val)
        .unwrap();

    Ok(res_ptr.into())
}

/// @bufGet(buf, i) — read one byte as Integer; OOB / stale → Unit sentinel.
pub fn call_builtin_macro_buf_get<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "bufGet expects 2 arguments".to_string(),
            help: None,
        });
    }
    let buf_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let buf_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, buf_ptr, 1, "buf_data_ptr")
        .unwrap();
    let buf_handle = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), buf_data_ptr, "buf_handle")
        .unwrap()
        .into_int_value();

    let idx_ptr = self_compiler
        .compile_expr(&args[1], module)?
        .into_pointer_value();
    let idx_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, idx_ptr, 1, "idx_data_ptr")
        .unwrap();
    let index_int = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), idx_data_ptr, "idx_int")
        .unwrap()
        .into_int_value();

    let get_fn = self_compiler.get_runtime_fn(module, "__buffer_get")?;
    let get_call = self_compiler
        .builder
        .build_call(
            get_fn,
            &[buf_handle.into(), index_int.into()],
            "buffer_get_call",
        )
        .unwrap();

    match get_call.try_as_basic_value() {
        ValueKind::Basic(val) => {
            let res_ptr = create_entry_block_alloca(self_compiler, "buf_get_res_alloc")?;
            self_compiler.builder.build_store(res_ptr, val).unwrap();
            Ok(res_ptr.into())
        }
        ValueKind::Instruction(_) => Err(SprsError::Internal {
            message: "Expected basic value from __buffer_get".to_string(),
            location: None,
        }),
    }
}

/// @bufSet(buf, i, v) — write byte `v` (low 8 bits) at index `i`. OOB → no-op.
pub fn call_builtin_macro_buf_set<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 3 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "bufSet expects 3 arguments".to_string(),
            help: None,
        });
    }
    let buf_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let buf_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, buf_ptr, 1, "buf_data_ptr")
        .unwrap();
    let buf_handle = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), buf_data_ptr, "buf_handle")
        .unwrap()
        .into_int_value();

    let idx_ptr = self_compiler
        .compile_expr(&args[1], module)?
        .into_pointer_value();
    let idx_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, idx_ptr, 1, "idx_data_ptr")
        .unwrap();
    let index_int = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), idx_data_ptr, "idx_int")
        .unwrap()
        .into_int_value();

    let v_ptr = self_compiler
        .compile_expr(&args[2], module)?
        .into_pointer_value();
    let v_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, v_ptr, 1, "v_data_ptr")
        .unwrap();
    let v_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), v_data_ptr, "v_int")
        .unwrap()
        .into_int_value();

    let set_fn = self_compiler.get_runtime_fn(module, "__buffer_set")?;
    self_compiler
        .builder
        .build_call(
            set_fn,
            &[buf_handle.into(), index_int.into(), v_val.into()],
            "buffer_set_call",
        )
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "buf_set_res_alloc")?;
    self_compiler.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "unit_res");
    Ok(res_ptr.into())
}

/// `@raw(buf)` — move Buffer ownership to a RawPtr. Requires `unsafe`.
/// Source binding becomes Unit; caller must `@free` the result.
pub fn call_builtin_macro_raw<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if self_compiler.unsafe_depth == 0 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 15,
            },
            location: Location::new(
                String::new(),
                args.first().map(|a| a.span).unwrap_or(Span::DUMMY),
            ),
            message: "`@raw` requires an unsafe block".to_string(),
            help: Some("wrap the call in `unsafe { ... }`".to_string()),
        });
    }
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@raw expects 1 argument".to_string(),
            help: None,
        });
    }
    let buf_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let buf_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            buf_ptr,
            1,
            "raw_buf_data_ptr",
        )
        .unwrap();
    let buf_handle = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            buf_data_ptr,
            "raw_buf_handle",
        )
        .unwrap()
        .into_int_value();

    let into_raw_fn = self_compiler.get_runtime_fn(module, "__buffer_into_raw")?;
    let ptr = match self_compiler
        .builder
        .build_call(into_raw_fn, &[buf_handle.into()], "buffer_into_raw_call")
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__buffer_into_raw returned void".to_string(),
                location: None,
            });
        }
    };

    if let ast::Expr::Var(name) = &args[0].node {
        if let Some(binding) = self_compiler.get_variables(name) {
            let src_ptr = binding.value.into_pointer_value();
            let tag_ptr = self_compiler
                .builder
                .build_struct_gep(
                    self_compiler.runtime_value_type,
                    src_ptr,
                    0,
                    "raw_src_tag_ptr",
                )
                .unwrap();
            // Invalidate the source binding so auto-drop / destroy cannot double-free.
            self_compiler
                .builder
                .build_store(
                    tag_ptr,
                    self_compiler
                        .context
                        .i32_type()
                        .const_int(Tag::Unit as u64, false),
                )
                .unwrap();
        }
    }

    let res_ptr = create_entry_block_alloca(self_compiler, "raw_ptr_res_alloc")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::RawPtr as u64),
        StoreValue::Int(ptr),
        "raw_ptr_res_store",
    );
    Ok(res_ptr.into())
}

/// `@free(p)` — release a RawPtr from `@raw`. Requires `unsafe`.
/// Null/unknown are no-ops; source binding becomes Unit.
pub fn call_builtin_macro_free<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if self_compiler.unsafe_depth == 0 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 15,
            },
            location: Location::new(
                String::new(),
                args.first().map(|a| a.span).unwrap_or(Span::DUMMY),
            ),
            message: "`@free` requires an unsafe block".to_string(),
            help: Some("wrap the call in `unsafe { ... }`".to_string()),
        });
    }
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@free expects 1 argument".to_string(),
            help: None,
        });
    }
    let p_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let p_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            p_ptr,
            1,
            "free_p_data_ptr",
        )
        .unwrap();
    let p_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), p_data_ptr, "free_p")
        .unwrap()
        .into_int_value();

    let free_fn = self_compiler.get_runtime_fn(module, "__raw_free")?;
    self_compiler
        .builder
        .build_call(free_fn, &[p_val.into()], "raw_free_call")
        .unwrap();

    if let ast::Expr::Var(name) = &args[0].node {
        if let Some(binding) = self_compiler.get_variables(name) {
            let src_ptr = binding.value.into_pointer_value();
            let tag_ptr = self_compiler
                .builder
                .build_struct_gep(
                    self_compiler.runtime_value_type,
                    src_ptr,
                    0,
                    "free_src_tag_ptr",
                )
                .unwrap();
            // Prevent reuse of a freed RawPtr binding.
            self_compiler
                .builder
                .build_store(
                    tag_ptr,
                    self_compiler
                        .context
                        .i32_type()
                        .const_int(Tag::Unit as u64, false),
                )
                .unwrap();
        }
    }

    let res_ptr = create_entry_block_alloca(self_compiler, "free_res_alloc")?;
    self_compiler.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "free_unit");
    Ok(res_ptr.into())
}

pub fn call_builtin_macro_clone<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@clone expects 1 argument".to_string(),
            help: None,
        });
    }
    let arg_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();

    let result_ptr = clone_runtime_value(self_compiler, arg_ptr, module)?;
    Ok(result_ptr.into())
}

pub fn call_builtin_macro_move<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@move expects 1 argument".to_string(),
            help: None,
        });
    }

    let name = match &args[0].node {
        ast::Expr::Var(name) => name,
        _ => {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 13,
                },
                location: Location::new(String::new(), args[0].span),
                message: "@move expects a variable argument".to_string(),
                help: None,
            });
        }
    };

    let source = self_compiler
        .get_variables(name)
        .ok_or_else(|| SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 2,
            },
            location: Location::new(String::new(), args[0].span),
            message: format!("Undefined variable: {}", name),
            help: None,
        })?;

    if !source.always_clone {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), args[0].span),
            message: format!("@move expects a cp variable: {}", name),
            help: None,
        });
    }

    let source_ptr = source.value.into_pointer_value();
    let moved_ptr = var_load_at_init_variable(self_compiler, source_ptr, "move_arg")?;
    move_variable(self_compiler, &source_ptr.into(), name);
    Ok(moved_ptr.into())
}

pub fn call_builtin_macro_cast<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@cast expects 2 arguments".to_string(),
            help: None,
        });
    }

    let value_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let target_type_expr = &args[1];

    let target_type = match &target_type_expr.node {
        ast::Expr::Var(ident) => ident.as_str(),
        ast::Expr::TypeI8 => "i8",
        ast::Expr::TypeU8 => "u8",
        ast::Expr::TypeI16 => "i16",
        ast::Expr::TypeU16 => "u16",
        ast::Expr::TypeI32 => "i32",
        ast::Expr::TypeU32 => "u32",
        ast::Expr::TypeI64 => "i64",
        ast::Expr::TypeU64 => "u64",

        ast::Expr::TypeF16 => "fp16",
        ast::Expr::TypeF32 => "fp32",
        ast::Expr::TypeF64 => "fp64",
        _ => {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 8,
                },
                location: Location::new(String::new(), target_type_expr.span),
                message: format!(
                    "@cast second argument must be a type identifier : {:?}",
                    target_type_expr
                ),
                help: None,
            });
        }
    };

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            value_ptr,
            0,
            "cast_arg_tag_ptr",
        )
        .unwrap();

    // Load the current tag (not used here but could be useful for type checking)
    let current_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), tag_ptr, "cast_arg_tag")
        .unwrap()
        .into_int_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            value_ptr,
            1,
            "cast_arg_data_ptr",
        )
        .unwrap();
    let data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "cast_arg_data")
        .unwrap()
        .into_int_value();

    let parent = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let bb_int = self_compiler
        .context
        .append_basic_block(parent, "cast_int_bb");
    let bb_float = self_compiler
        .context
        .append_basic_block(parent, "cast_float_bb");
    let bb_f16 = self_compiler
        .context
        .append_basic_block(parent, "cast_f16_bb");
    let bb_f32 = self_compiler
        .context
        .append_basic_block(parent, "cast_f32_bb");
    let bb_f64 = self_compiler
        .context
        .append_basic_block(parent, "cast_f64_bb");
    let bb_uint = self_compiler
        .context
        .append_basic_block(parent, "cast_uint_bb");
    let marge = self_compiler
        .context
        .append_basic_block(parent, "cast_merge_bb");
    let error_bb = self_compiler
        .context
        .append_basic_block(parent, "cast_error_bb");
    let final_merge = self_compiler
        .context
        .append_basic_block(parent, "cast_final_merge_bb");

    let i32_type = self_compiler.context.i32_type();

    // short-circuit: if the input is an error label, return it directly.
    let input_is_error = build_label_is_error(self_compiler, current_tag, data, module)?;
    let input_error_bb = self_compiler
        .context
        .append_basic_block(parent, "cast_input_error");
    let cast_normal_bb = self_compiler
        .context
        .append_basic_block(parent, "cast_normal");
    let _ = self_compiler.builder.build_conditional_branch(
        input_is_error,
        input_error_bb,
        cast_normal_bb,
    );

    self_compiler.builder.position_at_end(input_error_bb);
    let _ = self_compiler
        .builder
        .build_unconditional_branch(final_merge);

    self_compiler.builder.position_at_end(cast_normal_bb);

    let cases = vec![
        // Signed integers -> SITOFP (bb_int)
        (i32_type.const_int(Tag::Integer as u64, false), bb_int),
        (i32_type.const_int(Tag::Int8 as u64, false), bb_int),
        (i32_type.const_int(Tag::Int16 as u64, false), bb_int),
        (i32_type.const_int(Tag::Int32 as u64, false), bb_int),
        (i32_type.const_int(Tag::Int64 as u64, false), bb_int),
        // Unsigned integers -> UITOFP (bb_uint)
        (i32_type.const_int(Tag::Uint8 as u64, false), bb_uint),
        (i32_type.const_int(Tag::Uint16 as u64, false), bb_uint),
        (i32_type.const_int(Tag::Uint32 as u64, false), bb_uint),
        (i32_type.const_int(Tag::Uint64 as u64, false), bb_uint),
        // Floats -> f64
        (i32_type.const_int(Tag::Float as u64, false), bb_float),
        (i32_type.const_int(Tag::Float16 as u64, false), bb_f16),
        (i32_type.const_int(Tag::Float32 as u64, false), bb_f32),
        (i32_type.const_int(Tag::Float64 as u64, false), bb_f64),
    ];

    self_compiler
        .builder
        .build_switch(current_tag, error_bb, &cases)
        .unwrap();

    // error branch: unknown tag → error label
    self_compiler.builder.position_at_end(error_bb);
    let error_ptr =
        create_error_label_from_str(self_compiler, "TypeError: unexpected tag in @cast", module)?;
    let _ = self_compiler
        .builder
        .build_unconditional_branch(final_merge);

    // Integer -> f64
    self_compiler.builder.position_at_end(bb_int);
    let int_to_f64 = self_compiler
        .builder
        .build_signed_int_to_float(data, self_compiler.context.f64_type(), "int_to_f64")
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();
    // Unsigned Integer -> f64 (UITOFP)
    self_compiler.builder.position_at_end(bb_uint);
    let uint_to_f64 = self_compiler
        .builder
        .build_unsigned_int_to_float(data, self_compiler.context.f64_type(), "uint_to_f64")
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float -> f64
    self_compiler.builder.position_at_end(bb_float);
    let float_to_f64 = self_compiler
        .builder
        .build_bit_cast(data, self_compiler.context.f64_type(), "float_to_f64")
        .unwrap()
        .into_float_value();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float16 -> f64
    self_compiler.builder.position_at_end(bb_f16);
    let f16_to_f64 = self_compiler
        .builder
        .build_int_truncate(data, self_compiler.context.i16_type(), "f16_to_f64")
        .unwrap();
    let val_f16 = self_compiler
        .builder
        .build_bit_cast(
            f16_to_f64,
            self_compiler.context.f16_type(),
            "f16_to_f64_cast",
        )
        .unwrap()
        .into_float_value();

    let val_f16_ext = self_compiler
        .builder
        .build_float_ext(val_f16, self_compiler.context.f64_type(), "f16_to_f64_ext")
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float32 -> f64
    self_compiler.builder.position_at_end(bb_f32);
    let val_f32_i32 = self_compiler
        .builder
        .build_int_truncate(data, self_compiler.context.i32_type(), "f32_to_f64")
        .unwrap();
    let val_f32 = self_compiler
        .builder
        .build_bit_cast(
            val_f32_i32,
            self_compiler.context.f32_type(),
            "f32_to_f64_cast",
        )
        .unwrap()
        .into_float_value();
    let val_f32_ext = self_compiler
        .builder
        .build_float_ext(val_f32, self_compiler.context.f64_type(), "f32_to_f64_ext")
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float64 -> f64
    self_compiler.builder.position_at_end(bb_f64);
    let val_f64 = self_compiler
        .builder
        .build_bit_cast(data, self_compiler.context.f64_type(), "f64_to_f64")
        .unwrap()
        .into_float_value();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Merge block
    self_compiler.builder.position_at_end(marge);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.context.f64_type(), "cast_phi")
        .unwrap();
    phi.add_incoming(&[
        (&int_to_f64, bb_int),
        (&uint_to_f64, bb_uint),
        (&float_to_f64, bb_float),
        (&val_f16_ext, bb_f16),
        (&val_f32_ext, bb_f32),
        (&val_f64, bb_f64),
    ]);
    let normalized_f64 = phi.as_basic_value().into_float_value();

    let (new_tag, new_data) = match target_type {
        "i8" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int8 as u64, false);

            let new_data = self_compiler
                .builder
                .build_float_to_signed_int(
                    normalized_f64,
                    self_compiler.context.i8_type(),
                    "cast_to_int8",
                )
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_s_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_int8_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "u8" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint8 as u64, false);

            let new_data = self_compiler
                .builder
                .build_float_to_unsigned_int(
                    normalized_f64,
                    self_compiler.context.i8_type(),
                    "cast_to_uint8",
                )
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_uint8_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "i16" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int16 as u64, false);

            let new_data = self_compiler
                .builder
                .build_float_to_signed_int(
                    normalized_f64,
                    self_compiler.context.i16_type(),
                    "cast_to_int16",
                )
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_s_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_int16_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "u16" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint16 as u64, false);

            let new_data = self_compiler
                .builder
                .build_float_to_unsigned_int(
                    normalized_f64,
                    self_compiler.context.i16_type(),
                    "cast_to_uint16",
                )
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_uint16_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "i32" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int32 as u64, false);

            let new_data = self_compiler
                .builder
                .build_float_to_signed_int(
                    normalized_f64,
                    self_compiler.context.i32_type(),
                    "cast_to_int32",
                )
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_s_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_int32_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "u32" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint32 as u64, false);

            let new_data = self_compiler
                .builder
                .build_float_to_unsigned_int(
                    normalized_f64,
                    self_compiler.context.i32_type(),
                    "cast_to_uint32",
                )
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_uint32_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "i64" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int64 as u64, false);
            let new_data = self_compiler
                .builder
                .build_float_to_signed_int(
                    normalized_f64,
                    self_compiler.context.i64_type(),
                    "cast_to_int64",
                )
                .unwrap();
            (new_tag, new_data)
        }
        "u64" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint64 as u64, false);
            let new_data = self_compiler
                .builder
                .build_float_to_unsigned_int(
                    normalized_f64,
                    self_compiler.context.i64_type(),
                    "cast_to_uint64",
                )
                .unwrap();
            (new_tag, new_data)
        }

        "fp16" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float16 as u64, false);

            // f64 -> f16

            let new_data = self_compiler
                .builder
                .build_float_trunc(
                    normalized_f64,
                    self_compiler.context.f16_type(),
                    "cast_to_fp16",
                )
                .unwrap();

            let new_data_i16 = self_compiler
                .builder
                .build_bit_cast(new_data, self_compiler.context.i16_type(), "fp16_to_i16")
                .unwrap()
                .into_int_value();

            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data_i16,
                    self_compiler.context.i64_type(),
                    "cast_to_fp16_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }

        "fp32" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float32 as u64, false);

            // f64 -> f32

            let new_data = self_compiler
                .builder
                .build_float_trunc(
                    normalized_f64,
                    self_compiler.context.f32_type(),
                    "cast_to_fp32",
                )
                .unwrap();

            let new_data_i32 = self_compiler
                .builder
                .build_bit_cast(new_data, self_compiler.context.i32_type(), "fp32_to_i32")
                .unwrap()
                .into_int_value();

            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data_i32,
                    self_compiler.context.i64_type(),
                    "cast_to_fp32_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }

        "fp64" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float64 as u64, false);

            let new_data = self_compiler
                .builder
                .build_bit_cast(
                    normalized_f64,
                    self_compiler.context.i64_type(),
                    "cast_to_fp64_ext",
                )
                .unwrap()
                .into_int_value();
            (new_tag, new_data)
        }
        _ => {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 9,
                },
                location: Location::new(String::new(), Span::DUMMY),
                message: format!("Unsupported target type for @cast: {:?}", target_type),
                help: None,
            });
        }
    };

    let result_ptr = create_entry_block_alloca(self_compiler, "cast_res_alloc")?;
    self_compiler.build_runtime_value_store(
        result_ptr,
        StoreTag::Dynamic(new_tag),
        StoreValue::Int(new_data),
        "cast_res",
    );
    let success_end_bb = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler
        .builder
        .build_unconditional_branch(final_merge);

    self_compiler.builder.position_at_end(final_merge);
    let final_phi = self_compiler
        .builder
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            "cast_final_phi",
        )
        .unwrap();
    final_phi.add_incoming(&[
        (&error_ptr, error_bb),
        (&result_ptr, success_end_bb),
        (&value_ptr, input_error_bb),
    ]);
    return Ok(final_phi.as_basic_value());
}

pub fn call_builtin_macro_lshift<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@lshift expects 2 arguments (value, shift_amount)".to_string(),
            help: None,
        });
    }
    shift_impl(self_compiler, args, module, ShiftDir::Left)
}

pub fn call_builtin_macro_rshift<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@rshift expects 2 arguments (value, shift_amount)".to_string(),
            help: None,
        });
    }
    shift_impl(self_compiler, args, module, ShiftDir::Right)
}

enum ShiftDir {
    Left,
    Right,
}

fn shift_impl<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
    dir: ShiftDir,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let value_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let shift_ptr = self_compiler
        .compile_expr(&args[1], module)?
        .into_pointer_value();

    let rvt = self_compiler.runtime_value_type;
    let i64_type = self_compiler.context.i64_type();

    // Load value tag + data
    let value_tag_ptr = self_compiler
        .builder
        .build_struct_gep(rvt, value_ptr, 0, "lshift_val_tag_ptr")
        .unwrap();
    let value_tag = self_compiler
        .builder
        .build_load(
            self_compiler.context.i32_type(),
            value_tag_ptr,
            "lshift_val_tag",
        )
        .unwrap()
        .into_int_value();
    let value_data_ptr = self_compiler
        .builder
        .build_struct_gep(rvt, value_ptr, 1, "lshift_val_data_ptr")
        .unwrap();
    let value_data = self_compiler
        .builder
        .build_load(i64_type, value_data_ptr, "lshift_val_data")
        .unwrap()
        .into_int_value();

    // Load shift amount tag + data
    let shift_tag_ptr = self_compiler
        .builder
        .build_struct_gep(rvt, shift_ptr, 0, "shift_amt_tag_ptr")
        .unwrap();
    let shift_tag = self_compiler
        .builder
        .build_load(
            self_compiler.context.i32_type(),
            shift_tag_ptr,
            "shift_amt_tag",
        )
        .unwrap()
        .into_int_value();
    let shift_data_ptr = self_compiler
        .builder
        .build_struct_gep(rvt, shift_ptr, 1, "lshift_amt_data_ptr")
        .unwrap();
    let shift_amt = self_compiler
        .builder
        .build_load(i64_type, shift_data_ptr, "lshift_amt_data")
        .unwrap()
        .into_int_value();

    let parent = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let bb_signed = self_compiler
        .context
        .append_basic_block(parent, "shift_signed_bb");
    let bb_unsigned = self_compiler
        .context
        .append_basic_block(parent, "shift_unsigned_bb");
    let bb_err = self_compiler
        .context
        .append_basic_block(parent, "shift_err_bb");
    let marge = self_compiler
        .context
        .append_basic_block(parent, "shift_merge_bb");
    let final_merge = self_compiler
        .context
        .append_basic_block(parent, "shift_final_merge_bb");

    let i32_type = self_compiler.context.i32_type();

    // short-circuit: if either operand is an error label, return it directly.
    let val_is_error = build_label_is_error(self_compiler, value_tag, value_data, module)?;
    let val_error_bb = self_compiler
        .context
        .append_basic_block(parent, "shift_val_error_short_circuit");
    let check_shift_error_bb = self_compiler
        .context
        .append_basic_block(parent, "shift_check_shift_error");
    let _ = self_compiler.builder.build_conditional_branch(
        val_is_error,
        val_error_bb,
        check_shift_error_bb,
    );

    self_compiler.builder.position_at_end(val_error_bb);
    let _ = self_compiler
        .builder
        .build_unconditional_branch(final_merge);

    self_compiler.builder.position_at_end(check_shift_error_bb);
    let shift_is_error = build_label_is_error(self_compiler, shift_tag, shift_amt, module)?;
    let shift_error_bb = self_compiler
        .context
        .append_basic_block(parent, "shift_amt_error_short_circuit");
    let shift_normal_bb = self_compiler
        .context
        .append_basic_block(parent, "shift_normal_dispatch");
    let _ = self_compiler.builder.build_conditional_branch(
        shift_is_error,
        shift_error_bb,
        shift_normal_bb,
    );

    self_compiler.builder.position_at_end(shift_error_bb);
    let _ = self_compiler
        .builder
        .build_unconditional_branch(final_merge);

    self_compiler.builder.position_at_end(shift_normal_bb);

    // Signed integer tags: Integer, Int8, Int16, Int32, Int64
    let signed_cases = vec![
        (i32_type.const_int(Tag::Integer as u64, false), bb_signed),
        (i32_type.const_int(Tag::Int8 as u64, false), bb_signed),
        (i32_type.const_int(Tag::Int16 as u64, false), bb_signed),
        (i32_type.const_int(Tag::Int32 as u64, false), bb_signed),
        (i32_type.const_int(Tag::Int64 as u64, false), bb_signed),
    ];
    // Unsigned integer tags: Uint8, Uint16, Uint32, Uint64
    let unsigned_cases = vec![
        (i32_type.const_int(Tag::Uint8 as u64, false), bb_unsigned),
        (i32_type.const_int(Tag::Uint16 as u64, false), bb_unsigned),
        (i32_type.const_int(Tag::Uint32 as u64, false), bb_unsigned),
        (i32_type.const_int(Tag::Uint64 as u64, false), bb_unsigned),
    ];
    let mut all_cases = signed_cases;
    all_cases.extend(unsigned_cases);

    // Default -> error (non-integer tag)
    self_compiler
        .builder
        .build_switch(value_tag, bb_err, &all_cases)
        .unwrap();

    // Signed: for lshift use shl, for rshift use ashr (sign-fill)
    self_compiler.builder.position_at_end(bb_signed);
    let signed_result = match dir {
        ShiftDir::Left => self_compiler
            .builder
            .build_left_shift(value_data, shift_amt, "lshift_signed")
            .unwrap(),
        ShiftDir::Right => self_compiler
            .builder
            .build_right_shift(value_data, shift_amt, true, "rshift_signed")
            .unwrap(),
    };
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Unsigned: for lshift use shl, for rshift use lshr (zero-fill)
    self_compiler.builder.position_at_end(bb_unsigned);
    let unsigned_result = match dir {
        ShiftDir::Left => self_compiler
            .builder
            .build_left_shift(value_data, shift_amt, "lshift_unsigned")
            .unwrap(),
        ShiftDir::Right => self_compiler
            .builder
            .build_right_shift(value_data, shift_amt, false, "rshift_unsigned")
            .unwrap(),
    };
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Error: non-integer tag — create error label
    self_compiler.builder.position_at_end(bb_err);
    let err_msg: &'static str = match dir {
        ShiftDir::Left => "@lshift expects an integer value",
        ShiftDir::Right => "@rshift expects an integer value",
    };
    let error_ptr = create_error_label_from_str(self_compiler, err_msg, module)?;
    self_compiler
        .builder
        .build_unconditional_branch(final_merge)
        .unwrap();

    self_compiler.builder.position_at_end(marge);
    let phi = self_compiler
        .builder
        .build_phi(i64_type, "shift_phi")
        .unwrap();
    phi.add_incoming(&[(&signed_result, bb_signed), (&unsigned_result, bb_unsigned)]);
    let shifted = phi.as_basic_value().into_int_value();

    let result_ptr = create_entry_block_alloca(self_compiler, "shift_res")?;
    self_compiler.build_runtime_value_store(
        result_ptr,
        StoreTag::Dynamic(value_tag),
        StoreValue::Int(shifted),
        "shift_res_store",
    );
    let success_end_bb = self_compiler.builder.get_insert_block().unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(final_merge)
        .unwrap();

    self_compiler.builder.position_at_end(final_merge);
    let final_phi = self_compiler
        .builder
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            "shift_final_phi",
        )
        .unwrap();
    final_phi.add_incoming(&[
        (&error_ptr, bb_err),
        (&result_ptr, success_end_bb),
        (&value_ptr, val_error_bb),
        (&shift_ptr, shift_error_bb),
    ]);
    Ok(final_phi.as_basic_value())
}

pub fn call_builtin_macro_not<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 13,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@not expects 1 argument".to_string(),
            help: None,
        });
    }

    let value_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();

    let rvt = self_compiler.runtime_value_type;
    let i64_type = self_compiler.context.i64_type();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(rvt, value_ptr, 1, "not_data_ptr")
        .unwrap();
    let data = self_compiler
        .builder
        .build_load(i64_type, data_ptr, "not_data")
        .unwrap()
        .into_int_value();

    let zero = i64_type.const_int(0, false);
    let negated = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, data, zero, "not_result")
        .unwrap();

    let result_ptr = create_entry_block_alloca(self_compiler, "not_res")?;
    self_compiler.build_runtime_value_store(
        result_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Int(
            self_compiler
                .builder
                .build_int_z_extend(negated, i64_type, "not_zext")
                .unwrap(),
        ),
        "not_res_store",
    );
    Ok(result_ptr.into())
}

/// @is_error(x) — returns true (1) if x is a Label named "error", false (0) otherwise.
pub fn call_builtin_macro_is_error<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 3,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@is_error expects exactly 1 argument".to_string(),
            help: None,
        });
    }

    let val_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();

    // Delegate to `__label_is_error(tag, data)`: the data handle alone can
    // false-positive on immediate integers, so both fields are loaded.
    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            0,
            "is_error_tag_ptr",
        )
        .unwrap();
    let tag_val = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), tag_ptr, "is_error_tag")
        .unwrap()
        .into_int_value();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            1,
            "is_error_data_ptr",
        )
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "is_error_data")
        .unwrap()
        .into_int_value();

    let is_error = build_label_is_error(self_compiler, tag_val, data_val, module)?;

    // Store as a Bool runtime_value.
    let res_ptr = create_entry_block_alloca(self_compiler, "is_error_res")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Int(
            self_compiler
                .builder
                .build_int_z_extend(is_error, self_compiler.context.i64_type(), "is_error_zext")
                .unwrap(),
        ),
        "is_error_res_store",
    );
    Ok(res_ptr.into())
}

/// @error_message(x) — returns the error reason as a String value.
/// Error labels with a String payload return that payload; other error-label
/// payloads are rendered via `format_sprs_value`; non-errors return "".
pub fn call_builtin_macro_error_message<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 3,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@error_message expects exactly 1 argument".to_string(),
            help: None,
        });
    }

    let val_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            1,
            "error_msg_data_ptr",
        )
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "error_msg_data")
        .unwrap()
        .into_int_value();

    let error_msg_fn = self_compiler.get_runtime_fn(module, "__error_message_from_label")?;
    let string_handle = match self_compiler
        .builder
        .build_call(error_msg_fn, &[data_val.into()], "error_msg_call")
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__error_message_from_label returned void".to_string(),
                location: None,
            });
        }
    };

    // Store as a String runtime_value.
    let res_ptr = create_entry_block_alloca(self_compiler, "error_msg_res")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::String as u64),
        StoreValue::Int(string_handle),
        "error_msg_res_store",
    );
    Ok(res_ptr.into())
}
/// @attach(expr, :name) captures a cloned value for later `:name` uses.
pub fn call_builtin_macro_attach<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 3,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@attach expects exactly 2 arguments: expression and label".to_string(),
            help: None,
        });
    }
    let slot_name = match &args[1].node {
        ast::Expr::AttachSlot(name) => name.clone(),
        _ => {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 3,
                },
                location: Location::new(String::new(), args[1].span),
                message: "@attach second argument must be a slot such as <:name".to_string(),
                help: None,
            });
        }
    };

    let value_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let captured_ptr = clone_runtime_value(self_compiler, value_ptr, module)?;
    if let Some(previous) = self_compiler
        .attachments
        .insert(slot_name.clone(), captured_ptr)
    {
        let drop_fn = self_compiler.get_runtime_fn(module, "__drop")?;
        crate::llvm::variable::drop_var(
            self_compiler,
            previous,
            drop_fn,
            &format!("attach_{}", slot_name),
        );
    }
    create_unit(self_compiler)
}

/// @label_is(value, expected) — true if value is a label whose name matches expected.
/// `expected` must be an atom (`:name` or `:"{i}-item"`).
pub fn call_builtin_macro_label_is<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 3,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@label_is expects exactly 2 arguments: value and label".to_string(),
            help: None,
        });
    }

    let expected_name = match &args[1].node {
        ast::Expr::Atom(name) => name,
        _ => {
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 3,
                },
                location: Location::new(String::new(), args[1].span),
                message:
                    "@label_is second argument must be an atom such as :name or :\"{i}-item\""
                        .to_string(),
                help: None,
            });
        }
    };

    let val_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            1,
            "label_is_data_ptr",
        )
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "label_is_data")
        .unwrap()
        .into_int_value();

    let cmp_i32 = match expected_name {
        LabelName::Static(static_name) => {
            let idx = self_compiler.string_counter;
            self_compiler.string_counter += 1;
            let name_ptr = self_compiler
                .builder
                .build_global_string_ptr(static_name, &format!("label_is_name_{}", idx))
                .unwrap()
                .as_pointer_value();
            let name_eq = self_compiler.get_runtime_fn(module, "__label_name_eq")?;
            match self_compiler
                .builder
                .build_call(
                    name_eq,
                    &[
                        data_val.into(),
                        name_ptr.into(),
                        self_compiler
                            .context
                            .i64_type()
                            .const_int(static_name.len() as u64, false)
                            .into(),
                    ],
                    "label_is_name_eq",
                )
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                _ => {
                    return Err(SprsError::Internal {
                        message: "__label_name_eq returned void".to_string(),
                        location: None,
                    });
                }
            }
        }
        LabelName::Dynamic(parts) => {
            // Build the expected name string, intern it as an atom, and
            // compare ids with the actual label's name (also interned).
            // No temporary Label is created (Labels now require a payload).
            let atom_from_string = self_compiler.get_runtime_fn(module, "__atom_from_string")?;
            let (expected_str, temps_to_drop) =
                build_dynamic_string(self_compiler, parts, module)?;
            let expected_id = match self_compiler
                .builder
                .build_call(
                    atom_from_string,
                    &[expected_str.into()],
                    "label_is_expected_atom",
                )
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                _ => {
                    return Err(SprsError::Internal {
                        message: "__atom_from_string returned void".to_string(),
                        location: None,
                    });
                }
            };
            let label_name_fn = self_compiler.get_runtime_fn(module, "__label_name")?;
            let actual_name = match self_compiler
                .builder
                .build_call(label_name_fn, &[data_val.into()], "label_is_actual_name")
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                _ => {
                    return Err(SprsError::Internal {
                        message: "__label_name returned void".to_string(),
                        location: None,
                    });
                }
            };
            let actual_id = match self_compiler
                .builder
                .build_call(
                    atom_from_string,
                    &[actual_name.into()],
                    "label_is_actual_atom",
                )
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                _ => {
                    return Err(SprsError::Internal {
                        message: "__atom_from_string returned void".to_string(),
                        location: None,
                    });
                }
            };
            let atom_eq = self_compiler.get_runtime_fn(module, "__atom_eq")?;
            let cmp = match self_compiler
                .builder
                .build_call(
                    atom_eq,
                    &[expected_id.into(), actual_id.into()],
                    "label_is_atom_eq",
                )
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                _ => {
                    return Err(SprsError::Internal {
                        message: "__atom_eq returned void".to_string(),
                        location: None,
                    });
                }
            };
            // Drop temporaries: the expected-name string chain and the
            // actual label's name string.
            let drop_fn = self_compiler.get_runtime_fn(module, "__drop")?;
            let string_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::String as u64, false);
            for (temporary_index, temp) in temps_to_drop.into_iter().enumerate() {
                self_compiler
                    .builder
                    .build_call(
                        drop_fn,
                        &[string_tag.into(), temp.into()],
                        &format!("label_is_drop_tmp_{}", temporary_index),
                    )
                    .unwrap();
            }
            self_compiler
                .builder
                .build_call(
                    drop_fn,
                    &[string_tag.into(), actual_name.into()],
                    "label_is_drop_actual_name",
                )
                .unwrap();
            cmp
        }
    };

    let res_ptr = create_entry_block_alloca(self_compiler, "label_is_res")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Int(
            self_compiler
                .builder
                .build_int_z_extend(cmp_i32, self_compiler.context.i64_type(), "label_is_zext")
                .unwrap(),
        ),
        "label_is_res_store",
    );
    Ok(res_ptr.into())
}

/// @label_payload(value) — clone the label payload. Non-label → Unit.
pub fn call_builtin_macro_label_payload<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 3,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@label_payload expects exactly 1 argument".to_string(),
            help: None,
        });
    }

    let val_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            1,
            "label_payload_data_ptr",
        )
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            data_ptr,
            "label_payload_data",
        )
        .unwrap()
        .into_int_value();

    let payload_fn = self_compiler.get_runtime_fn(module, "__label_payload")?;
    let call_site = self_compiler
        .builder
        .build_call(payload_fn, &[data_val.into()], "label_payload_call")
        .unwrap();
    let result_val = match call_site.try_as_basic_value() {
        ValueKind::Basic(val) => val,
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__label_payload returned void".to_string(),
                location: None,
            });
        }
    };

    let res_ptr = create_entry_block_alloca(self_compiler, "label_payload_res")?;
    self_compiler
        .builder
        .build_store(res_ptr, result_val)
        .unwrap();
    Ok(res_ptr.into())
}

/// @label_name(value) — return the label name as String. Non-label → "".
pub fn call_builtin_macro_label_name<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 3,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@label_name expects exactly 1 argument".to_string(),
            help: None,
        });
    }

    let val_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            1,
            "label_name_data_ptr",
        )
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            data_ptr,
            "label_name_data",
        )
        .unwrap()
        .into_int_value();

    let name_fn = self_compiler.get_runtime_fn(module, "__label_name")?;
    let string_handle = match self_compiler
        .builder
        .build_call(name_fn, &[data_val.into()], "label_name_call")
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__label_name returned void".to_string(),
                location: None,
            });
        }
    };

    let res_ptr = create_entry_block_alloca(self_compiler, "label_name_res")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::String as u64),
        StoreValue::Int(string_handle),
        "label_name_res_store",
    );
    Ok(res_ptr.into())
}

/// @error(reason) — creates a `{:error, reason}` label.
/// reason: any expression; the compiled value becomes the label payload.
pub fn call_builtin_macro_error<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 3,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@error expects exactly 1 argument: reason".to_string(),
            help: None,
        });
    }

    create_label(
        self_compiler,
        &LabelName::Static("error".into()),
        &args[0],
        module,
    )
}

use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use inkwell::{
    AddressSpace,
    module::Linkage,
    values::{BasicValueEnum, IntValue, PointerValue, ValueKind},
};

use crate::llvm::variable::{clone_runtime_value, move_variable};
use crate::{
    front::ast,
    front::span::Spanned,
    llvm::compiler::{Compiler, StoreTag, StoreValue, StrConstantResult, Tag},
    llvm::data_structures::create_unit,
};

pub struct ErrorValueSettings {
    pub is_const: bool,
    pub is_global: bool,
}

/// Generate IR that creates a `Tag::Error` value in the slab.
/// Stores the result as a runtime_value_type `{ i32 tag=Error, i64 data=handle }`
/// in a fresh alloca and returns the pointer.
/// The caller should NOT emit `build_unreachable` — the error value flows
/// through normal control flow so callers can propagate or catch it.
pub fn create_error_value<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    error_code: u32,
    message: &str,
    module: &inkwell::module::Module<'ctx>,
    settings: ErrorValueSettings,
) -> Result<PointerValue<'ctx>, SprsError> {
    // Store the message string as a global constant.
    let global = self_compiler.set_global_constant_str(
        module,
        message,
        settings.is_global,
        settings.is_const,
    );

    let (msg_ptr, msg_len) = match global {
        Some(StrConstantResult::Global(g)) => {
            let ptr = g.as_pointer_value();
            let ptr_i8 = self_compiler.builder.build_bit_cast(
                ptr,
                self_compiler.context.ptr_type(AddressSpace::default()),
                "error_msg_ptr_i8",
            );
            (ptr_i8.unwrap().into_pointer_value(), message.len() as u64)
        }
        Some(StrConstantResult::Pointer(p)) => (p, message.len() as u64),
        None => {
            // Empty message — pass null pointer.
            let null_ptr = self_compiler
                .context
                .ptr_type(AddressSpace::default())
                .const_null();
            (null_ptr, 0u64)
        }
    };

    let error_code_val = self_compiler
        .context
        .i32_type()
        .const_int(error_code as u64, false);
    let msg_len_val = self_compiler.context.i64_type().const_int(msg_len, false);

    let error_new_fn = self_compiler.get_runtime_fn(module, "__error_new")?;
    let error_handle = match self_compiler
        .builder
        .build_call(
            error_new_fn,
            &[error_code_val.into(), msg_ptr.into(), msg_len_val.into()],
            "error_new_call",
        )
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__error_new returned void".to_string(),
                location: None,
            });
        }
    };

    // Store as a runtime_value_type { tag: Error, data: handle }
    let res_ptr = create_entry_block_alloca(self_compiler, "error_val")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Error as u64),
        StoreValue::Int(error_handle),
        "error_val_store",
    );

    Ok(res_ptr)
}

pub(crate) fn create_entry_block_alloca<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    name: &str,
) -> Result<PointerValue<'ctx>, SprsError> {
    let builder = &self_compiler.builder;
    let current_block = builder.get_insert_block().ok_or(SprsError::Internal {
        message: "no insert block".to_string(),
        location: None,
    })?;
    let function = current_block.get_parent().ok_or(SprsError::Internal {
        message: "no parent function".to_string(),
        location: None,
    })?;
    let entry_block = function
        .get_first_basic_block()
        .ok_or(SprsError::Internal {
            message: "no entry block".to_string(),
            location: None,
        })?;

    match entry_block.get_first_instruction() {
        Some(first_instr) => builder.position_before(&first_instr),
        None => builder.position_at_end(entry_block),
    }

    let alloca = builder
        .build_alloca(
            self_compiler.runtime_value_type,
            format!("{}_var_alloca", name).as_str(),
        )
        .unwrap();

    builder.position_at_end(current_block);
    Ok(alloca)
}

// !normal functions

pub fn create_list_from_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    elements: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let len = elements.len();
    let i64_type = self_compiler.context.i64_type();

    let list_new_fn = self_compiler.get_runtime_fn(module, "__list_new")?;

    let list_call = self_compiler
        .builder
        .build_call(
            list_new_fn,
            &[i64_type.const_int(len as u64, false).into()],
            "list_new_call",
        )
        .unwrap();

    // `__list_new` returns an i64 handle (not a pointer).
    let list_handle = match list_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => {
            return Err(SprsError::Internal {
                message: "Expected i64 handle from __list_new".to_string(),
                location: None,
            });
        }
    };

    let list_push_fn = self_compiler.get_runtime_fn(module, "__list_push")?;
    for elem in elements {
        let compiled_val_ptr = self_compiler
            .compile_expr(elem, module)?
            .into_pointer_value();
        let (val_ptr, source_var) = if let ast::Expr::Var(name) = &elem.node {
            let (source_ptr, _, source_always_clone) = self_compiler
                .get_variables(name)
                .ok_or_else(|| format!("Undefined variable: {}", name))?;

            if source_always_clone {
                (
                    clone_runtime_value(self_compiler, source_ptr.into_pointer_value(), module)?,
                    None,
                )
            } else {
                (compiled_val_ptr, Some((source_ptr, name)))
            }
        } else {
            (compiled_val_ptr, None)
        };

        // `__list_push(list_handle: i64, tag: i32, data: i64)` — pass the
        // handle as the first arg, with tag/data extracted from the value.
        self_compiler.build_sprs_value_call_func(
            val_ptr,
            list_push_fn,
            "list_push",
            &[list_handle.into()],
            true,
        );

        if let Some((source_ptr, source_name)) = source_var {
            move_variable(self_compiler, &source_ptr, source_name);
        }
    }
    Ok(list_handle)
}

pub fn create_integer<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    n: &i64,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(self_compiler, "num_alloc")?;

    self_compiler.build_runtime_value_store(
        ptr,
        StoreTag::Int(Tag::Integer as u64),
        StoreValue::Int(self_compiler.context.i64_type().const_int(*n as u64, true)),
        "int",
    );

    Ok(ptr.into())
}

pub fn create_float<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    f: f64,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(self_compiler, "float_alloc")?;

    self_compiler.build_runtime_value_store(
        ptr,
        StoreTag::Int(Tag::Float as u64),
        StoreValue::Float(f),
        "float",
    );

    Ok(ptr.into())
}

pub fn create_string<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    str: &String,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let idx = self_compiler.string_counter;
    self_compiler.string_counter += 1;
    let str_val = self_compiler.context.const_string(str.as_bytes(), true);
    let global = module.add_global(
        str_val.get_type(),
        Some(AddressSpace::default()),
        &format!("str_const_{}", idx),
    );
    global.set_initializer(&str_val);
    global.set_linkage(Linkage::Internal);
    global.set_constant(true);

    // Build a runtime String slot that owns a proper Rust `String` (with
    // length tracking — no NUL-termination assumption). The slot is freed
    // by `__drop` on scope exit, fixing BUG-R04 (String leak) and BUG-R05
    // (NUL-terminated buffer over-read in `__clone`).
    let string_from_cstr_fn = self_compiler.get_runtime_fn(module, "__string_from_cstr")?;
    let cstr_ptr = global.as_pointer_value();
    let string_call = self_compiler
        .builder
        .build_call(
            string_from_cstr_fn,
            &[cstr_ptr.into()],
            "string_from_cstr_call",
        )
        .unwrap();
    let string_handle = match string_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => {
            return Err(SprsError::Internal {
                message: "Expected i64 handle from __string_from_cstr".to_string(),
                location: None,
            });
        }
    };

    let ptr = create_entry_block_alloca(self_compiler, "str_alloc")?;

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "str_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::String as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "str_data_ptr")
        .unwrap();
    // `string_handle` is already an i64 — no ptr_to_int needed.
    self_compiler
        .builder
        .build_store(data_ptr, string_handle)
        .unwrap();

    Ok(ptr.into())
}

pub fn create_bool<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    boolean: &bool,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(self_compiler, "bool_alloc")?;

    self_compiler.build_runtime_value_store(
        ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Int(
            self_compiler
                .context
                .i64_type()
                .const_int(if *boolean { 1 } else { 0 }, false),
        ),
        "bool",
    );

    Ok(ptr.into())
}

pub fn create_typed_zero<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    tag: Tag,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(self_compiler, &format!("{}_alloc", name))?;
    self_compiler.build_runtime_value_store(
        ptr,
        StoreTag::Int(tag as u64),
        StoreValue::Int(self_compiler.context.i64_type().const_int(0, false)),
        name,
    );
    Ok(ptr.into())
}

pub fn create_int8<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Int8, "int8")
}
pub fn create_uint8<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Uint8, "uint8")
}
pub fn create_int16<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Int16, "int16")
}
pub fn create_uint16<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Uint16, "uint16")
}
pub fn create_int32<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Int32, "int32")
}
pub fn create_uint32<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Uint32, "uint32")
}
pub fn create_int64<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Int64, "int64")
}
pub fn create_uint64<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Uint64, "uint64")
}
pub fn create_float16<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Float16, "f16")
}
pub fn create_float32<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Float32, "f32")
}
pub fn create_float64<'ctx>(c: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(c, Tag::Float64, "f64")
}

pub fn create_dummy_for_no_return<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<(), SprsError> {
    let dummy = create_entry_block_alloca(self_compiler, "ret_dummy")?;
    self_compiler.build_runtime_value_store(
        dummy,
        StoreTag::Int(Tag::Unit as u64),
        StoreValue::Int(self_compiler.context.i64_type().const_int(0, false)),
        "ret_dummy",
    );

    let val = self_compiler
        .builder
        .build_load(self_compiler.runtime_value_type, dummy, "ret_dummy_val")
        .unwrap();
    self_compiler.builder.build_return(Some(&val)).unwrap();
    Ok(())
}

pub(crate) fn box_return_value<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    module: &inkwell::module::Module<'ctx>,
    return_type: inkwell::types::BasicTypeEnum<'ctx>,
    result_val: BasicValueEnum<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let result_ptr = create_entry_block_alloca(self_compiler, "compile_expr_call_res_alloc")?;

    if return_type.is_int_type() {
        let int_type = return_type.into_int_type();
        let int_val = result_val.into_int_value();

        // boolean case
        if int_type.get_bit_width() == 1 {
            let bool_as_i64 = self_compiler
                .builder
                .build_int_z_extend(int_val, self_compiler.context.i64_type(), "bool_to_i64")
                .unwrap();

            self_compiler.build_runtime_value_store(
                result_ptr,
                StoreTag::Int(Tag::Boolean as u64),
                StoreValue::Int(bool_as_i64),
                "res_boolean",
            );
            return Ok(result_ptr.into());
        } else {
            let val_i64 = self_compiler
                .builder
                .build_int_s_extend(int_val, self_compiler.context.i64_type(), "int_to_i64")
                .unwrap();

            self_compiler.build_runtime_value_store(
                result_ptr,
                StoreTag::Int(Tag::Integer as u64),
                StoreValue::Int(val_i64),
                "res_integer",
            );
        }
    } else if return_type.is_float_type() {
        let float_val = result_val.into_float_value();

        let val_f64 = self_compiler
            .builder
            .build_float_ext(float_val, self_compiler.context.f64_type(), "float_to_f64")
            .unwrap();

        let data = self_compiler
            .builder
            .build_bit_cast(val_f64, self_compiler.context.i64_type(), "f64_to_i64")
            .unwrap()
            .into_int_value();

        self_compiler.build_runtime_value_store(
            result_ptr,
            StoreTag::Int(Tag::Float as u64),
            StoreValue::Int(data),
            "res_float",
        );
    } else if return_type.is_struct_type() {
        self_compiler
            .builder
            .build_store(result_ptr, result_val)
            .unwrap();
    } else if return_type.is_pointer_type() {
        // Extern function returning `i8*` (a C string). Register the pointer
        // in a slab String slot so the runtime owns it properly.
        let ptr_val = result_val.into_pointer_value();
        let string_from_cstr_fn = self_compiler.get_runtime_fn(module, "__string_from_cstr")?;
        let string_call = self_compiler
            .builder
            .build_call(
                string_from_cstr_fn,
                &[ptr_val.into()],
                "string_from_cstr_call",
            )
            .unwrap();
        let string_handle = match string_call.try_as_basic_value() {
            ValueKind::Basic(val) => val.into_int_value(),
            _ => {
                return Err(SprsError::Internal {
                    message: "Expected i64 handle from __string_from_cstr".to_string(),
                    location: None,
                });
            }
        };

        self_compiler.build_runtime_value_store(
            result_ptr,
            StoreTag::Int(Tag::String as u64),
            StoreValue::Int(string_handle),
            "res_string",
        );
    } else {
        self_compiler.tag_only_runtime_value_store(result_ptr, Tag::Unit as u64, "res_unit");
    };
    Ok(result_ptr.into())
}

pub fn create_call_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    ident: &str,
    args: &Vec<Spanned<ast::Expr>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let func = module
        .get_function(ident)
        .or_else(|| {
            self_compiler
                .modules
                .values()
                .find_map(|m| m.get_function(ident))
        })
        .ok_or(SprsError::Semantic {
            code: ErrorCode {
                category: ErrorCategory::Semantic,
                number: 15,
            },
            location: Location::new(String::new(), Span::DUMMY),
            message: format!("Undefined function: {}", ident),
            help: None,
        })?;
    let mut compiled_args = Vec::with_capacity(args.len());
    for arg in args {
        let compiled_arg_ptr = self_compiler
            .compile_expr(arg, module)?
            .into_pointer_value();
        let (arg_ptr, source_var) = if let ast::Expr::Var(name) = &arg.node {
            let (source_ptr, _, source_always_clone) = self_compiler
                .get_variables(name)
                .ok_or_else(|| format!("Undefined variable: {}", name))?;

            if source_always_clone {
                (
                    clone_runtime_value(self_compiler, source_ptr.into_pointer_value(), module)?,
                    None,
                )
            } else {
                (compiled_arg_ptr, Some((source_ptr, name)))
            }
        } else {
            (compiled_arg_ptr, None)
        };

        let temp_arg_ptr = create_entry_block_alloca(self_compiler, "compile_expr_arg_alloc")?;
        let val_tag_ptr = self_compiler
            .builder
            .build_struct_gep(self_compiler.runtime_value_type, arg_ptr, 0, "val_tag_ptr")
            .unwrap();
        let val_data_ptr = self_compiler
            .builder
            .build_struct_gep(self_compiler.runtime_value_type, arg_ptr, 1, "val_data_ptr")
            .unwrap();
        let val_tag = self_compiler
            .builder
            .build_load(self_compiler.context.i32_type(), val_tag_ptr, "val_tag")
            .unwrap();
        let val_data = self_compiler
            .builder
            .build_load(self_compiler.context.i64_type(), val_data_ptr, "val_data")
            .unwrap();

        let temp_tag_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                temp_arg_ptr,
                0,
                "temp_tag_ptr",
            )
            .unwrap();
        let temp_data_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                temp_arg_ptr,
                1,
                "temp_data_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(temp_tag_ptr, val_tag)
            .unwrap();
        self_compiler
            .builder
            .build_store(temp_data_ptr, val_data)
            .unwrap();
        compiled_args.push(temp_arg_ptr.into());

        if let Some((source_ptr, source_name)) = source_var {
            move_variable(self_compiler, &source_ptr, source_name);
        }
    }
    let call_site = self_compiler
        .builder
        .build_call(func, &compiled_args, "compile_expr_call_tmp")
        .unwrap();

    let return_type_opt = func.get_type().get_return_type();
    if return_type_opt.is_none() {
        return create_unit(self_compiler);
    }
    let return_type = return_type_opt.unwrap();
    let result_val = match call_site.try_as_basic_value() {
        ValueKind::Basic(val) => val,
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "Expected basic value from function call".to_string(),
                location: None,
            });
        }
    };
    box_return_value(self_compiler, module, return_type, result_val)
}

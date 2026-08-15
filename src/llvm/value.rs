use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use inkwell::{
    AddressSpace,
    module::Linkage,
    values::{BasicMetadataValueEnum, BasicValueEnum, IntValue, PointerValue, ValueKind},
};

use crate::llvm::variable::{clone_runtime_value, move_variable};
use crate::{
    front::ast,
    front::label_name::{LabelName, LabelNamePart},
    front::span::Spanned,
    front::type_helper::Type,
    llvm::compiler::{Compiler, StoreTag, StoreValue, StrConstantResult, Tag},
    llvm::data_structures::create_unit,
};

/// Generate IR that creates a `Label` named "error" whose payload is a String
/// slot built from `message`. Stores the result as a runtime_value_type
/// `{ i32 tag=Label, i64 data=handle }` in a fresh alloca and returns the
/// pointer. The caller should NOT emit `build_unreachable` — the error label
/// flows through normal control flow so callers can propagate or catch it.
pub fn create_error_label_from_str<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    message: &str,
    module: &inkwell::module::Module<'ctx>,
) -> Result<PointerValue<'ctx>, SprsError> {
    // Store the message string as a global constant.
    let global = self_compiler.set_global_constant_str(module, message, true, true);

    let (msg_ptr, msg_len) = match global {
        Some(StrConstantResult::Global(global_value)) => {
            let ptr = global_value.as_pointer_value();
            let ptr_i8 = self_compiler.builder.build_bit_cast(
                ptr,
                self_compiler.context.ptr_type(AddressSpace::default()),
                "error_msg_ptr_i8",
            );
            (ptr_i8.unwrap().into_pointer_value(), message.len() as u64)
        }
        Some(StrConstantResult::Pointer(pointer_value)) => (pointer_value, message.len() as u64),
        None => {
            // Empty message — pass null pointer.
            let null_ptr = self_compiler
                .context
                .ptr_type(AddressSpace::default())
                .const_null();
            (null_ptr, 0u64)
        }
    };

    let msg_len_val = self_compiler.context.i64_type().const_int(msg_len, false);

    let error_label_fn = self_compiler.get_runtime_fn(module, "__error_label_from_str")?;
    let handle = match self_compiler
        .builder
        .build_call(
            error_label_fn,
            &[msg_ptr.into(), msg_len_val.into()],
            "error_label_call",
        )
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__error_label_from_str returned void".to_string(),
                location: None,
            });
        }
    };

    // Store as a runtime_value_type { tag: Label, data: handle }
    let res_ptr = create_entry_block_alloca(self_compiler, "error_label_val")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Label as u64),
        StoreValue::Int(handle),
        "error_label_val_store",
    );

    Ok(res_ptr)
}

/// Emit `__label_is_error(tag, data)` and lower its i32 (0/1) result to an
/// i1 predicate. True iff the value is a Label named "error".
pub fn build_label_is_error<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    tag: IntValue<'ctx>,
    data: IntValue<'ctx>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let label_is_error_fn = self_compiler.get_runtime_fn(module, "__label_is_error")?;
    let result = match self_compiler
        .builder
        .build_call(
            label_is_error_fn,
            &[tag.into(), data.into()],
            "label_is_error_call",
        )
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        ValueKind::Instruction(_) => {
            return Err(SprsError::Internal {
                message: "__label_is_error returned void".to_string(),
                location: None,
            });
        }
    };
    Ok(self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::NE,
            result,
            self_compiler.context.i32_type().const_int(0, false),
            "label_is_error_pred",
        )
        .unwrap())
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
    number_value: &i64,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(self_compiler, "num_alloc")?;

    self_compiler.build_runtime_value_store(
        ptr,
        StoreTag::Int(Tag::Integer as u64),
        StoreValue::Int(self_compiler.context.i64_type().const_int(*number_value as u64, true)),
        "int",
    );

    Ok(ptr.into())
}

pub fn create_float<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    float_value: f64,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(self_compiler, "float_alloc")?;

    self_compiler.build_runtime_value_store(
        ptr,
        StoreTag::Int(Tag::Float as u64),
        StoreValue::Float(float_value),
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

pub fn create_label<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    name: &LabelName,
    payload: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let initial_payload_ptr = self_compiler
        .compile_expr(payload, module)?
        .into_pointer_value();

    let mut source_to_move: Option<(BasicValueEnum<'ctx>, String)> = None;
    let final_payload_ptr = if let ast::Expr::Var(source_name) = &payload.node {
        if let Some(source) = self_compiler.get_variables(source_name) {
            if source.always_clone {
                clone_runtime_value(self_compiler, source.value.into_pointer_value(), module)?
            } else {
                source_to_move = Some((source.value, source_name.clone()));
                initial_payload_ptr
            }
        } else {
            initial_payload_ptr
        }
    } else {
        initial_payload_ptr
    };

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            final_payload_ptr,
            0,
            "label_payload_tag_ptr",
        )
        .unwrap();
    let tag = self_compiler
        .builder
        .build_load(
            self_compiler.context.i32_type(),
            tag_ptr,
            "label_payload_tag",
        )
        .unwrap()
        .into_int_value();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            final_payload_ptr,
            1,
            "label_payload_data_ptr",
        )
        .unwrap();
    let data = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            data_ptr,
            "label_payload_data",
        )
        .unwrap()
        .into_int_value();

    let handle = match name {
        LabelName::Static(static_name) => {
            let label_index = self_compiler.string_counter;
            self_compiler.string_counter += 1;
            let name_ptr = self_compiler
                .builder
                .build_global_string_ptr(static_name, &format!("label_name_{}", label_index))
                .unwrap()
                .as_pointer_value();
            let label_new = self_compiler.get_runtime_fn(module, "__label_new")?;
            match self_compiler
                .builder
                .build_call(
                    label_new,
                    &[
                        name_ptr.into(),
                        self_compiler
                            .context
                            .i64_type()
                            .const_int(static_name.len() as u64, false)
                            .into(),
                        tag.into(),
                        data.into(),
                    ],
                    "label_new_call",
                )
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(value) => value.into_int_value(),
                ValueKind::Instruction(_) => {
                    return Err(SprsError::Internal {
                        message: "__label_new returned void".to_string(),
                        location: None,
                    });
                }
            }
        }
        LabelName::Dynamic(parts) => {
            build_dynamic_label_handle(self_compiler, parts, tag, data, module)?
        }
    };

    let result_ptr = create_entry_block_alloca(self_compiler, "label_res")?;
    self_compiler.build_runtime_value_store(
        result_ptr,
        StoreTag::Int(Tag::Label as u64),
        StoreValue::Int(handle),
        "label_res_store",
    );

    if let Some((source_ptr, source_name)) = source_to_move {
        move_variable(self_compiler, &source_ptr, &source_name);
    }
    Ok(result_ptr.into())
}

/// Create an immutable atom (`Tag::Atom`, data = interned id).
///
/// Static names go through `__atom_from_bytes` directly; dynamic templates
/// build the name string with [`build_dynamic_string`] and intern it via
/// `__atom_from_string`. Atoms never touch the attachment table.
pub fn create_atom<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    name: &LabelName,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let atom_id = match name {
        LabelName::Static(static_name) => {
            let atom_index = self_compiler.string_counter;
            self_compiler.string_counter += 1;
            let name_ptr = self_compiler
                .builder
                .build_global_string_ptr(static_name, &format!("atom_name_{}", atom_index))
                .unwrap()
                .as_pointer_value();
            let atom_from_bytes = self_compiler.get_runtime_fn(module, "__atom_from_bytes")?;
            match self_compiler
                .builder
                .build_call(
                    atom_from_bytes,
                    &[
                        name_ptr.into(),
                        self_compiler
                            .context
                            .i64_type()
                            .const_int(static_name.len() as u64, false)
                            .into(),
                    ],
                    "atom_from_bytes_call",
                )
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(value) => value.into_int_value(),
                ValueKind::Instruction(_) => {
                    return Err(SprsError::Internal {
                        message: "__atom_from_bytes returned void".to_string(),
                        location: None,
                    });
                }
            }
        }
        LabelName::Dynamic(parts) => {
            let (acc, temps_to_drop) = build_dynamic_string(self_compiler, parts, module)?;
            let atom_from_string = self_compiler.get_runtime_fn(module, "__atom_from_string")?;
            let id = match self_compiler
                .builder
                .build_call(atom_from_string, &[acc.into()], "atom_from_string_call")
                .unwrap()
                .try_as_basic_value()
            {
                ValueKind::Basic(value) => value.into_int_value(),
                ValueKind::Instruction(_) => {
                    return Err(SprsError::Internal {
                        message: "__atom_from_string returned void".to_string(),
                        location: None,
                    });
                }
            };
            // Drop all temporary string handles (atom interned the name).
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
                        &format!("atom_drop_tmp_{}", temporary_index),
                    )
                    .unwrap();
            }
            id
        }
    };

    let result_ptr = create_entry_block_alloca(self_compiler, "atom_res")?;
    self_compiler.build_runtime_value_store(
        result_ptr,
        StoreTag::Int(Tag::Atom as u64),
        StoreValue::Int(atom_id),
        "atom_res_store",
    );
    Ok(result_ptr.into())
}

/// Build a dynamic name string from template parts.
///
/// Returns `(final string handle, temporary handles to drop)`; the final
/// handle is included in the temporaries. Callers intern it into a Label or
/// Atom, then drop the temporaries.
pub(crate) fn build_dynamic_string<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    parts: &[LabelNamePart],
    module: &inkwell::module::Module<'ctx>,
) -> Result<(IntValue<'ctx>, Vec<IntValue<'ctx>>), SprsError> {
    let string_new = self_compiler.get_runtime_fn(module, "__string_new")?;
    let string_concat = self_compiler.get_runtime_fn(module, "__string_concat")?;
    let value_to_string = self_compiler.get_runtime_fn(module, "__value_to_string")?;
    let panic_fn = self_compiler.get_runtime_fn(module, "__panic")?;

    // Start with empty string.
    let empty_ptr = self_compiler
        .builder
        .build_global_string_ptr("", "dyn_label_empty")
        .unwrap()
        .as_pointer_value();
    let mut acc = match self_compiler
        .builder
        .build_call(
            string_new,
            &[
                empty_ptr.into(),
                self_compiler.context.i64_type().const_int(0, false).into(),
            ],
            "dyn_label_empty_call",
        )
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(basic_value) => basic_value.into_int_value(),
        _ => {
            return Err(SprsError::Internal {
                message: "__string_new returned void".to_string(),
                location: None,
            });
        }
    };

    let mut temps_to_drop: Vec<IntValue<'ctx>> = vec![acc];

    for (part_idx, part) in parts.iter().enumerate() {
        let piece = match part {
            LabelNamePart::Lit(lit) => {
                let idx = self_compiler.string_counter;
                self_compiler.string_counter += 1;
                let lit_ptr = self_compiler
                    .builder
                    .build_global_string_ptr(lit, &format!("dyn_label_lit_{}", idx))
                    .unwrap()
                    .as_pointer_value();
                match self_compiler
                    .builder
                    .build_call(
                        string_new,
                        &[
                            lit_ptr.into(),
                            self_compiler
                                .context
                                .i64_type()
                                .const_int(lit.len() as u64, false)
                                .into(),
                        ],
                        &format!("dyn_label_lit_call_{}", part_idx),
                    )
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                    _ => {
                        return Err(SprsError::Internal {
                            message: "__string_new returned void".to_string(),
                            location: None,
                        });
                    }
                }
            }
            LabelNamePart::Ident(ident) => {
                let binding = self_compiler.get_variables(ident).ok_or_else(|| {
                    SprsError::Semantic {
                        code: ErrorCode {
                            category: ErrorCategory::Semantic,
                            number: 2,
                        },
                        location: Location::new(String::new(), Span::DUMMY),
                        message: format!(
                            "Undefined variable in dynamic label name: {}",
                            ident
                        ),
                        help: None,
                    }
                })?;
                match &binding.ty {
                    Type::Int
                    | Type::Bool
                    | Type::Str
                    | Type::Any
                    | Type::TypeI64 => {}
                    other => {
                        return Err(SprsError::Semantic {
                            code: ErrorCode {
                                category: ErrorCategory::Semantic,
                                number: 3,
                            },
                            location: Location::new(String::new(), Span::DUMMY),
                            message: format!(
                                "dynamic label name part `{}` has type {:?}; only int/bool/str allowed",
                                ident, other
                            ),
                            help: None,
                        });
                    }
                }
                let var_ptr = binding.value.into_pointer_value();
                let tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(
                        self_compiler.runtime_value_type,
                        var_ptr,
                        0,
                        &format!("dyn_label_ident_tag_ptr_{}", part_idx),
                    )
                    .unwrap();
                let tag_val = self_compiler
                    .builder
                    .build_load(
                        self_compiler.context.i32_type(),
                        tag_ptr,
                        &format!("dyn_label_ident_tag_{}", part_idx),
                    )
                    .unwrap()
                    .into_int_value();
                let data_ptr = self_compiler
                    .builder
                    .build_struct_gep(
                        self_compiler.runtime_value_type,
                        var_ptr,
                        1,
                        &format!("dyn_label_ident_data_ptr_{}", part_idx),
                    )
                    .unwrap();
                let data_val = self_compiler
                    .builder
                    .build_load(
                        self_compiler.context.i64_type(),
                        data_ptr,
                        &format!("dyn_label_ident_data_{}", part_idx),
                    )
                    .unwrap()
                    .into_int_value();
                let converted = match self_compiler
                    .builder
                    .build_call(
                        value_to_string,
                        &[tag_val.into(), data_val.into()],
                        &format!("dyn_label_to_str_{}", part_idx),
                    )
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                    _ => {
                        return Err(SprsError::Internal {
                            message: "__value_to_string returned void".to_string(),
                            location: None,
                        });
                    }
                };

                // Panic if conversion failed (INVALID_HANDLE == 0).
                let is_invalid = self_compiler
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        converted,
                        self_compiler.context.i64_type().const_int(0, false),
                        &format!("dyn_label_invalid_{}", part_idx),
                    )
                    .unwrap();
                let parent_fn = self_compiler
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();
                let panic_bb = self_compiler.context.append_basic_block(
                    parent_fn,
                    &format!("dyn_label_panic_{}", part_idx),
                );
                let ok_bb = self_compiler.context.append_basic_block(
                    parent_fn,
                    &format!("dyn_label_ok_{}", part_idx),
                );
                self_compiler
                    .builder
                    .build_conditional_branch(is_invalid, panic_bb, ok_bb)
                    .unwrap();
                self_compiler.builder.position_at_end(panic_bb);
                let msg = self_compiler
                    .builder
                    .build_global_string_ptr(
                        "dynamic label name part is not int/bool/str",
                        &format!("dyn_label_panic_msg_{}", part_idx),
                    )
                    .unwrap()
                    .as_pointer_value();
                self_compiler
                    .builder
                    .build_call(panic_fn, &[msg.into()], &format!("dyn_label_panic_call_{}", part_idx))
                    .unwrap();
                self_compiler.builder.build_unreachable().unwrap();
                self_compiler.builder.position_at_end(ok_bb);

                converted
            }
        };

        temps_to_drop.push(piece);
        let concatenated = match self_compiler
            .builder
            .build_call(
                string_concat,
                &[acc.into(), piece.into()],
                &format!("dyn_label_concat_{}", part_idx),
            )
            .unwrap()
            .try_as_basic_value()
        {
            ValueKind::Basic(basic_value) => basic_value.into_int_value(),
            _ => {
                return Err(SprsError::Internal {
                    message: "__string_concat returned void".to_string(),
                    location: None,
                });
            }
        };
        temps_to_drop.push(concatenated);
        acc = concatenated;
    }

    Ok((acc, temps_to_drop))
}

/// Build a dynamic label name string from template parts, then create the label.
/// Temporary string handles are explicitly dropped after use.
fn build_dynamic_label_handle<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    parts: &[LabelNamePart],
    payload_tag: IntValue<'ctx>,
    payload_data: IntValue<'ctx>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let label_new_from_string = self_compiler.get_runtime_fn(module, "__label_new_from_string")?;
    let drop_fn = self_compiler.get_runtime_fn(module, "__drop")?;
    let (acc, temps_to_drop) = build_dynamic_string(self_compiler, parts, module)?;

    let handle = match self_compiler
        .builder
        .build_call(
            label_new_from_string,
            &[acc.into(), payload_tag.into(), payload_data.into()],
            "label_new_from_string_call",
        )
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(basic_value) => basic_value.into_int_value(),
        _ => {
            return Err(SprsError::Internal {
                message: "__label_new_from_string returned void".to_string(),
                location: None,
            });
        }
    };

    // Drop all temporary string handles (label cloned the final name).
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
                &format!("dyn_label_drop_tmp_{}", temporary_index),
            )
            .unwrap();
    }

    Ok(handle)
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

pub fn create_int8<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Int8, "int8")
}
pub fn create_uint8<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Uint8, "uint8")
}
pub fn create_int16<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Int16, "int16")
}
pub fn create_uint16<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Uint16, "uint16")
}
pub fn create_int32<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Int32, "int32")
}
pub fn create_uint32<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Uint32, "uint32")
}
pub fn create_int64<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Int64, "int64")
}
pub fn create_uint64<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Uint64, "uint64")
}
pub fn create_float16<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Float16, "f16")
}
pub fn create_float32<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Float32, "f32")
}
pub fn create_float64<'ctx>(compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_typed_zero(compiler, Tag::Float64, "f64")
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

pub fn prepare_call_args<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<Vec<inkwell::values::BasicMetadataValueEnum<'ctx>>, SprsError> {
    let mut compiled_args = Vec::with_capacity(args.len());
    for arg in args {
        let compiled_arg_ptr = self_compiler
            .compile_expr(arg, module)?
            .into_pointer_value();
        let (arg_ptr, source_var) = if let ast::Expr::Var(name) = &arg.node {
            if let Some(src) = self_compiler.get_variables(name) {
                if src.always_clone {
                    (
                        clone_runtime_value(self_compiler, src.value.into_pointer_value(), module)?,
                        None,
                    )
                } else {
                    (compiled_arg_ptr, Some((src.value, name)))
                }
            } else {
                (compiled_arg_ptr, None)
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
    Ok(compiled_args)
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
                .find_map(|module_value| module_value.get_function(ident))
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

    self_compiler.check_call_arguments(ident, args)?;

    let compiled_args = prepare_call_args(self_compiler, args, module)?;
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

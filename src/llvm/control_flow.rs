use crate::front::error::{ErrorCategory, ErrorCode, SprsError};
use crate::front::type_helper::Type;
use inkwell::{
    values::{BasicValueEnum, IntValue, PointerValue, ValueKind},
    builder::Builder,
};
use crate::{
    front::ast,
    front::label_name::LabelName,
    front::span::{Span, Spanned},
    llvm::compiler::{Compiler, StrConstantResult, StoreTag, StoreValue, Tag},
};
use crate::llvm::builder_helper;
use crate::llvm::value::create_entry_block_alloca;

pub fn create_if_condition<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    cond: &Spanned<ast::Expr>,
    then_blk: &Vec<Spanned<ast::Stmt>>,
    else_blk: &Option<Vec<Spanned<ast::Stmt>>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<(), SprsError> {
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let then_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "then_bb");
    let else_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "else_bb");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "if_merge");

    let cond_val = self_compiler.compile_expr(cond, module)?;
    let cond_ptr = cond_val.into_pointer_value();
    let cond_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            cond_ptr,
            1,
            "cond_data_ptr",
        )
        .unwrap();
    let cond_loaded = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            cond_data_ptr,
            "cond_loaded",
        )
        .unwrap()
        .into_int_value();
    let zero = self_compiler.context.i64_type().const_int(0, false);
    let cond_bool = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::NE, cond_loaded, zero, "if_cond_bool")
        .unwrap();

    let _ = self_compiler
        .builder
        .build_conditional_branch(cond_bool, then_bb, else_bb);

    self_compiler.builder.position_at_end(then_bb);
    self_compiler.compile_block(then_blk, module)?;
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }

    self_compiler.builder.position_at_end(else_bb);
    if let Some(else_blk) = else_blk {
        self_compiler.compile_block(else_blk, module)?;
    }
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }

    self_compiler.builder.position_at_end(merge_bb);
    Ok(())
}

pub fn create_while_condition<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    cond: &Spanned<ast::Expr>,
    body: &Vec<Spanned<ast::Stmt>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<(), SprsError> {
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let cond_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "while_cond");
    let body_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "while_body");
    let after_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "while_after");

    let _ = self_compiler.builder.build_unconditional_branch(cond_bb);
    self_compiler.builder.position_at_end(cond_bb);
    let cond_val = self_compiler.compile_expr(cond, module)?;
    let cond_ptr = cond_val.into_pointer_value();

    let cond_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            cond_ptr,
            1,
            "cond_data_ptr",
        )
        .unwrap();
    let cond_loaded = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            cond_data_ptr,
            "cond_loaded",
        )
        .unwrap()
        .into_int_value();

    let zero = self_compiler.context.i64_type().const_int(0, false);
    let cond_bool = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::NE,
            cond_loaded,
            zero,
            "while_cond_bool",
        )
        .unwrap();

    let _ = self_compiler
        .builder
        .build_conditional_branch(cond_bool, body_bb, after_bb);

    self_compiler.builder.position_at_end(body_bb);
    self_compiler.compile_block(body, module)?;

    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(cond_bb);
    }

    self_compiler.builder.position_at_end(after_bb);
    Ok(())
}

pub fn create_if_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    cond: &Spanned<ast::Expr>,
    then_expr: &Spanned<ast::Expr>,
    else_expr: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let then_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "then_bb");
    let else_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "else_bb");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "if_merge");

    let cond_val = self_compiler.compile_expr(cond, module)?;
    let cond_ptr = cond_val.into_pointer_value();
    let cond_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            cond_ptr,
            1,
            "cond_data_ptr",
        )
        .unwrap();
    let cond_loaded = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            cond_data_ptr,
            "cond_loaded",
        )
        .unwrap()
        .into_int_value();
    let zero = self_compiler.context.i64_type().const_int(0, false);
    let cond_bool = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::NE, cond_loaded, zero, "if_cond_bool")
        .unwrap();

    let _ = self_compiler
        .builder
        .build_conditional_branch(cond_bool, then_bb, else_bb);

    self_compiler.builder.position_at_end(then_bb);
    let then_val = self_compiler.compile_expr(then_expr, module)?;
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }
    let then_bb_end = self_compiler.builder.get_insert_block().unwrap();

    // TODO: Handle case where else_expr, such as if (test) : ok() ? no();
    // TODO: Also  such as if (test) ok() orelse no();

    self_compiler.builder.position_at_end(else_bb);
    let else_val = self_compiler.compile_expr(else_expr, module)?;
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }
    let else_bb_end = self_compiler.builder.get_insert_block().unwrap();

    self_compiler.builder.position_at_end(merge_bb);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.runtime_value_type, "if_phi")
        .unwrap();

    // Add PHI incoming only if the block branches to merge_bb
    // (i.e. it does NOT end with a return/unreachable).
    if then_bb_end != merge_bb {
        if let Some(term) = then_bb_end.get_terminator() {
            if term.get_opcode() == inkwell::values::InstructionOpcode::Br {
                phi.add_incoming(&[(&then_val, then_bb_end)]);
            }
        }
    }
    if else_bb_end != merge_bb {
        if let Some(term) = else_bb_end.get_terminator() {
            if term.get_opcode() == inkwell::values::InstructionOpcode::Br {
                phi.add_incoming(&[(&else_val, else_bb_end)]);
            }
        }
    }

    Ok(phi.as_basic_value())
}

/// Compare a label handle's name to a static byte string.
///
/// Private helper shared by `@label_is`'s static branch and `match` arm
/// checks; returns the i32 result of `__label_name_eq`.
fn build_label_name_eq<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    data_val: IntValue<'ctx>,
    name: &str,
    module: &inkwell::module::Module<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let idx = self_compiler.string_counter;
    self_compiler.string_counter += 1;
    let name_ptr = self_compiler
        .builder
        .build_global_string_ptr(name, &format!("match_label_name_{}", idx))
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
                    .const_int(name.len() as u64, false)
                    .into(),
            ],
            "match_label_name_eq",
        )
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(basic_value) => Ok(basic_value.into_int_value()),
        _ => Err(SprsError::Internal {
            message: "__label_name_eq returned void".to_string(),
            location: None,
        }),
    }
}

/// `match` statement: check the scrutinee against each arm's pattern in
/// order and run the first matching body.
///
/// - `?(var name)` arms (`MatchArmBody::ExprBreak`) store the expression into
///   `name` (same clone/move rules as assignment) and branch to `merge_bb`.
/// - No-bind arms (`MatchArmBody::Block`) compile a statement block.
/// - No arm matches → `__panic("Match failed")` + `unreachable`.
///
/// Static types drive the checks: an `Atom`-typed scrutinee skips the tag
/// test for `:name` patterns, a `Label`-typed one skips it for both forms;
/// `Any`/unannotated scrutinees dispatch on the runtime tag so mixed
/// Atom/Label arms work.
pub fn create_match_stmt<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    scrutinee: &Spanned<ast::Expr>,
    bind: &Option<String>,
    arms: &Vec<ast::MatchArm>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<(), SprsError> {
    // --- Semantic validation (SEM-017) ---
    let scrut_ty = self_compiler.infer_type(scrutinee);
    let is_atom_static = matches!(scrut_ty, Type::AtomVal)
        || matches!(&scrut_ty, Type::App(name, _) if name == "Atom");
    let is_label_static = matches!(scrut_ty, Type::Label)
        || matches!(&scrut_ty, Type::App(name, _) if name == "Label");

    for arm in arms {
        match &arm.pat {
            ast::MatchPat::Name(LabelName::Dynamic(_))
            | ast::MatchPat::LabelPayload {
                name: LabelName::Dynamic(_),
                ..
            } => {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 17,
                    },
                    location: self_compiler.location(arm.span),
                    message: "match patterns must be static :name in v1".to_string(),
                    help: Some(
                        "dynamic :\"{i}-item\" patterns are not supported yet".to_string(),
                    ),
                });
            }
            ast::MatchPat::LabelPayload { .. } if is_atom_static => {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 17,
                    },
                    location: self_compiler.location(arm.span),
                    message: "payload pattern requires Label scrutinee".to_string(),
                    help: Some("use a plain :name pattern for Atom values".to_string()),
                });
            }
            _ => {}
        }
        match (&bind, &arm.body) {
            (Some(_), ast::MatchArmBody::ExprBreak(_))
            | (None, ast::MatchArmBody::Block(_)) => {}
            _ => {
                return Err(SprsError::Internal {
                    message: "match arm body does not match bind form".to_string(),
                    location: None,
                });
            }
        }
    }

    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    // Scrutinee value: tag + data.
    let val_ptr = self_compiler
        .compile_expr(scrutinee, module)?
        .into_pointer_value();
    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            0,
            "match_tag_ptr",
        )
        .unwrap();
    let tag_val = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), tag_ptr, "match_tag")
        .unwrap()
        .into_int_value();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            1,
            "match_data_ptr",
        )
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "match_data")
        .unwrap()
        .into_int_value();

    let atom_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Atom as u64, false);
    let label_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Label as u64, false);
    let zero_i32 = self_compiler.context.i32_type().const_int(0, false);

    // Result binding variable, declared in the surrounding scope so the code
    // after the match can read it (`return result`).
    if let Some(bind_name) = bind {
        let unit_expr = Spanned::new(ast::Expr::Unit(), Span::DUMMY);
        let init_val = self_compiler
            .compile_expr(&unit_expr, module)?
            .into_pointer_value();
        self_compiler.add_variable(
            bind_name.clone(),
            init_val.into(),
            Type::Unit,
            false,
            false,
            false,
        );
    }

    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "match_merge");

    let mut next_bb: Option<inkwell::basic_block::BasicBlock<'ctx>> = None;
    for arm in arms {
        let arm_bb = self_compiler
            .context
            .append_basic_block(parent_fn, "match_arm");
        let mismatch_bb = self_compiler
            .context
            .append_basic_block(parent_fn, "match_next");

        // The check for this arm starts where the previous one left off.
        if let Some(prev) = next_bb {
            self_compiler.builder.position_at_end(prev);
        }

        let (atom_cond, label_cond) = match &arm.pat {
            ast::MatchPat::Name(LabelName::Static(name)) => {
                // Atom name match: data holds an interned atom id.
                let idx = self_compiler.string_counter;
                self_compiler.string_counter += 1;
                let name_ptr = self_compiler
                    .builder
                    .build_global_string_ptr(name, &format!("match_atom_name_{}", idx))
                    .unwrap()
                    .as_pointer_value();
                let atom_from_bytes = self_compiler.get_runtime_fn(module, "__atom_from_bytes")?;
                let expected_id = match self_compiler
                    .builder
                    .build_call(
                        atom_from_bytes,
                        &[
                            name_ptr.into(),
                            self_compiler
                                .context
                                .i64_type()
                                .const_int(name.len() as u64, false)
                                .into(),
                        ],
                        "match_expected_atom",
                    )
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(basic_value) => basic_value.into_int_value(),
                    _ => {
                        return Err(SprsError::Internal {
                            message: "__atom_from_bytes returned void".to_string(),
                            location: None,
                        });
                    }
                };
                let atom_eq = self_compiler.get_runtime_fn(module, "__atom_eq")?;
                let eq_i32 = match self_compiler
                    .builder
                    .build_call(
                        atom_eq,
                        &[data_val.into(), expected_id.into()],
                        "match_atom_eq",
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
                let eq_bool = self_compiler
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        eq_i32,
                        zero_i32,
                        "match_atom_eq_bool",
                    )
                    .unwrap();
                let atom_c = if is_atom_static {
                    eq_bool
                } else {
                    let tag_bool = self_compiler
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            tag_val,
                            atom_tag,
                            "match_atom_tag",
                        )
                        .unwrap();
                    self_compiler
                        .builder
                        .build_and(tag_bool, eq_bool, "match_atom_cond")
                        .unwrap()
                };

                // Label name match: data holds a label slot handle.
                let name_eq_i32 = build_label_name_eq(self_compiler, data_val, name, module)?;
                let eq_bool = self_compiler
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        name_eq_i32,
                        zero_i32,
                        "match_label_eq_bool",
                    )
                    .unwrap();
                let label_c = if is_label_static {
                    eq_bool
                } else {
                    let tag_bool = self_compiler
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            tag_val,
                            label_tag,
                            "match_label_tag",
                        )
                        .unwrap();
                    self_compiler
                        .builder
                        .build_and(tag_bool, eq_bool, "match_label_cond")
                        .unwrap()
                };
                (Some(atom_c), Some(label_c))
            }
            ast::MatchPat::LabelPayload {
                name: LabelName::Static(name),
                ..
            } => {
                let name_eq_i32 = build_label_name_eq(self_compiler, data_val, name, module)?;
                let eq_bool = self_compiler
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::NE,
                        name_eq_i32,
                        zero_i32,
                        "match_label_eq_bool",
                    )
                    .unwrap();
                let label_c = if is_label_static {
                    eq_bool
                } else {
                    let tag_bool = self_compiler
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            tag_val,
                            label_tag,
                            "match_label_tag",
                        )
                        .unwrap();
                    self_compiler
                        .builder
                        .build_and(tag_bool, eq_bool, "match_label_cond")
                        .unwrap()
                };
                (None, Some(label_c))
            }
            _ => unreachable!("dynamic patterns rejected above"),
        };

        let cond = match (atom_cond, label_cond) {
            (Some(atom_c), Some(label_c)) => self_compiler
                .builder
                .build_or(atom_c, label_c, "match_cond")
                .unwrap(),
            (Some(atom_c), None) => atom_c,
            (None, Some(label_c)) => label_c,
            (None, None) => unreachable!("arm must check something"),
        };
        self_compiler
            .builder
            .build_conditional_branch(cond, arm_bb, mismatch_bb)
            .unwrap();

        // Arm body. The payload binder lives in its own scope so it is
        // dropped when the arm ends (unless moved into the bind variable).
        self_compiler.builder.position_at_end(arm_bb);
        self_compiler.enter_scope();
        if let ast::MatchPat::LabelPayload { binder, .. } = &arm.pat {
            if binder != "_" {
                let payload_fn = self_compiler.get_runtime_fn(module, "__label_payload")?;
                let call_site = self_compiler
                    .builder
                    .build_call(payload_fn, &[data_val.into()], "match_payload_call")
                    .unwrap();
                let payload = match call_site.try_as_basic_value() {
                    ValueKind::Basic(val) => val,
                    ValueKind::Instruction(_) => {
                        return Err(SprsError::Internal {
                            message: "__label_payload returned void".to_string(),
                            location: None,
                        });
                    }
                };
                let binder_ptr = create_entry_block_alloca(self_compiler, binder)?;
                self_compiler
                    .builder
                    .build_store(binder_ptr, payload)
                    .unwrap();
                self_compiler.add_variable(
                    binder.clone(),
                    binder_ptr.into(),
                    Type::Any,
                    false,
                    false,
                    false,
                );
            }
        }
        match &arm.body {
            ast::MatchArmBody::ExprBreak(expr) => {
                let bind_name = bind.as_deref().ok_or_else(|| SprsError::Internal {
                    message: "ExprBreak arm without bind".to_string(),
                    location: None,
                })?;
                // Same clone/move rules as Stmt::Assign.
                let mut val_ptr = self_compiler
                    .compile_expr(expr, module)?
                    .into_pointer_value();
                let mut source_to_move: Option<(BasicValueEnum<'ctx>, String)> = None;
                if let ast::Expr::Var(src_val_name) = &expr.node {
                    let src = self_compiler
                        .get_variables(src_val_name)
                        .ok_or_else(|| format!("Undefined variable: {}", src_val_name))?;
                    if src.always_clone {
                        val_ptr = builder_helper::clone_runtime_value(
                            self_compiler,
                            src.value.into_pointer_value(),
                            module,
                        )?;
                    } else {
                        source_to_move = Some((src.value, src_val_name.clone()));
                    }
                }
                let target = self_compiler
                    .get_variables(bind_name)
                    .ok_or_else(|| format!("Undefined variable: {}", bind_name))?;
                let target_ptr = target.value.into_pointer_value();
                let drop_fn = self_compiler.get_runtime_fn(module, "__drop")?;
                builder_helper::drop_var(self_compiler, target_ptr, drop_fn, bind_name);
                let new_val = self_compiler
                    .builder
                    .build_load(self_compiler.runtime_value_type, val_ptr, "match_bind_load")
                    .unwrap();
                self_compiler
                    .builder
                    .build_store(target_ptr, new_val)
                    .unwrap();
                if let Some((val, src_name)) = source_to_move {
                    builder_helper::move_variable(self_compiler, &val, &src_name);
                }
                let rhs_ty = self_compiler.infer_type(expr);
                if !target.is_annotated || target.is_ambi {
                    self_compiler.set_variable_type(bind_name, rhs_ty);
                }
                self_compiler
                    .builder
                    .build_unconditional_branch(merge_bb)
                    .unwrap();
            }
            ast::MatchArmBody::Block(stmts) => {
                self_compiler.compile_block(stmts, module)?;
                if self_compiler
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    self_compiler
                        .builder
                        .build_unconditional_branch(merge_bb)
                        .unwrap();
                }
            }
        }
        self_compiler.exit_scope(module)?;
        next_bb = Some(mismatch_bb);
    }

    // No arm matched: panic + unreachable.
    if let Some(final_next) = next_bb {
        self_compiler.builder.position_at_end(final_next);
        let panic_msg = self_compiler.set_global_constant_str(module, "Match failed", false, true);
        let panic_ptr = match panic_msg {
            Some(StrConstantResult::Global(global_value)) => global_value.as_pointer_value(),
            Some(StrConstantResult::Pointer(parameter)) => parameter,
            None => {
                return Err(SprsError::Internal {
                    message: "Failed to create panic message".to_string(),
                    location: None,
                });
            }
        };
        let panic_fn = self_compiler.get_runtime_fn(module, "__panic")?;
        self_compiler
            .builder
            .build_call(panic_fn, &[panic_ptr.into()], "match_failed_panic")
            .unwrap();
        self_compiler.builder.build_unreachable().unwrap();
    }

    self_compiler.builder.position_at_end(merge_bb);
    Ok(())
}

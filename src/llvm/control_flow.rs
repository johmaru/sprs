use crate::front::error::{ErrorCategory, ErrorCode, SprsError};
use crate::front::type_helper::Type;
use crate::llvm::builder_helper;
use crate::llvm::value::create_entry_block_alloca;
use crate::{
    front::ast,
    front::label_name::LabelName,
    front::span::{Span, Spanned},
    llvm::compiler::{Compiler, StoreTag, StoreValue, StrConstantResult, Tag},
};
use inkwell::{
    AddressSpace,
    builder::Builder,
    values::{BasicValueEnum, IntValue, PointerValue, ValueKind},
};
use std::collections::HashSet;

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
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            "if_phi",
        )
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

/// Validate match arm patterns (shared by statement and expression match).
///
/// SEM-017 rules, in order: dynamic `:"{i}-item"` names are rejected;
/// `{:name, binder}` needs a non-Atom scrutinee; `case _` must be the last
/// arm (it matches anything, so arms after it would be dead).
fn validate_match_patterns<'a, 'ctx>(
    self_compiler: &Compiler<'ctx>,
    arms_pats_spans: impl Iterator<Item = (&'a ast::MatchPat, Span)>,
    is_atom_static: bool,
) -> Result<(), SprsError> {
    let arms: Vec<(&ast::MatchPat, Span)> = arms_pats_spans.collect();
    let last = arms.len().saturating_sub(1);
    for (i, (pat, span)) in arms.iter().enumerate() {
        match pat {
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
                    location: self_compiler.location(*span),
                    message: "match patterns must be static :name in v1".to_string(),
                    help: Some("dynamic :\"{i}-item\" patterns are not supported yet".to_string()),
                });
            }
            ast::MatchPat::LabelPayload { .. } if is_atom_static => {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 17,
                    },
                    location: self_compiler.location(*span),
                    message: "payload pattern requires Label scrutinee".to_string(),
                    help: Some("use a plain :name pattern for Atom values".to_string()),
                });
            }
            ast::MatchPat::Name(LabelName::Static(name)) => {
                self_compiler.resolve_closed_label_member(name, *span)?;
            }
            ast::MatchPat::Wildcard if i != last => {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 17,
                    },
                    location: self_compiler.location(*span),
                    message: "case _ must be the last match arm".to_string(),
                    help: Some("move case _ to the end".to_string()),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Require a `Type::ClosedLabelSet` scrutinee's fully qualified member arms
/// to cover every member.
///
/// Open Atom / Label matches stay runtime-checked (`Match failed` panic);
/// only closed label sets get a compile-time exhaustiveness check. `case _`
/// satisfies it. Missing members are listed fully qualified, in declaration order.
fn check_closed_label_set_match_exhaustiveness<'a>(
    compiler: &Compiler,
    scrut_ty: &Type,
    arms: impl Iterator<Item = (&'a ast::MatchPat, Span)>,
) -> Result<(), SprsError> {
    let Type::ClosedLabelSet(set) = scrut_ty else {
        return Ok(());
    };
    let Some(frame_info) = compiler.closed_label_sets.get(set) else {
        return Ok(());
    };
    if frame_info.members.is_empty() {
        return Ok(());
    }

    let mut covered: HashSet<&str> = HashSet::new();
    let mut first_span: Option<Span> = None;
    for (pat, span) in arms {
        if first_span.is_none() {
            first_span = Some(span);
        }
        match pat {
            ast::MatchPat::Wildcard => return Ok(()),
            ast::MatchPat::Name(LabelName::Static(name)) => {
                covered.insert(name.as_str());
            }
            _ => {}
        }
    }

    let missing: Vec<String> = frame_info
        .members
        .iter()
        .map(|member| format!("{}.{}", set, member))
        .filter(|full_name| !covered.contains(full_name.as_str()))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let missing_str = missing.join(", ");
    Err(SprsError::Semantic {
        code: ErrorCode {
            category: ErrorCategory::Semantic,
            number: 17,
        },
        location: compiler.location(first_span.unwrap_or(Span::DUMMY)),
        message: format!("non-exhaustive match on {}; missing {}", set, missing_str),
        help: Some("add the missing members or a final case _".to_string()),
    })
}

/// Build the i1 condition for one arm pattern from the scrutinee's tag/data.
///
/// Static types prune the check: an `Atom`-typed scrutinee emits only the atom
/// name comparison for `:name` patterns (no Label branch), a `Label`-typed one
/// emits only the label name comparison, and `Any` scrutinees test the runtime
/// tag so mixed Atom/Label arms work. `case _` is always true.
fn match_arm_condition<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    _scrut_ty: &Type,
    pat: &ast::MatchPat,
    tag_val: IntValue<'ctx>,
    data_val: IntValue<'ctx>,
    is_atom_static: bool,
    is_label_static: bool,
    module: &inkwell::module::Module<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let zero_i32 = self_compiler.context.i32_type().const_int(0, false);
    let atom_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Atom as u64, false);
    let label_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Label as u64, false);

    let (atom_cond, label_cond) = match pat {
        ast::MatchPat::Name(LabelName::Static(name)) => {
            if is_label_static {
                // Label-typed scrutinee: no Atom branch at all.
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
                (None, Some(eq_bool))
            } else {
                // Atom branch: data holds an interned atom id.
                let intern_key = name.clone();
                let idx = self_compiler.string_counter;
                self_compiler.string_counter += 1;
                let name_ptr = self_compiler
                    .builder
                    .build_global_string_ptr(&intern_key, &format!("match_atom_name_{}", idx))
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
                                .const_int(intern_key.len() as u64, false)
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
                // Label branch: only when the scrutinee is not statically an Atom.
                let label_c = if is_atom_static {
                    None
                } else {
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
                    let tag_bool = self_compiler
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            tag_val,
                            label_tag,
                            "match_label_tag",
                        )
                        .unwrap();
                    Some(
                        self_compiler
                            .builder
                            .build_and(tag_bool, eq_bool, "match_label_cond")
                            .unwrap(),
                    )
                };
                (Some(atom_c), label_c)
            }
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
        ast::MatchPat::Wildcard => {
            let always = self_compiler.context.bool_type().const_int(1, false);
            (Some(always), None)
        }
        ast::MatchPat::Name(LabelName::Dynamic(_))
        | ast::MatchPat::LabelPayload {
            name: LabelName::Dynamic(_),
            ..
        } => unreachable!("dynamic patterns rejected by validate_match_patterns"),
    };

    match (atom_cond, label_cond) {
        (Some(atom_c), Some(label_c)) => Ok(self_compiler
            .builder
            .build_or(atom_c, label_c, "match_cond")
            .unwrap()),
        (Some(atom_c), None) => Ok(atom_c),
        (None, Some(label_c)) => Ok(label_c),
        (None, None) => unreachable!("arm must check something"),
    }
}

/// Bind a `{:name, binder}` arm's payload into `binder` (skipped for `_`).
fn bind_label_payload<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    pat: &ast::MatchPat,
    data_val: IntValue<'ctx>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<(), SprsError> {
    if let ast::MatchPat::LabelPayload { binder, .. } = pat {
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
            );
        }
    }
    Ok(())
}

/// Evaluate `expr` and store it into `dest_ptr`, applying the same move rules
/// as assignment / match bind (`ExprBreak`).
///
/// - `Expr::Var` → load/store then `move_variable` on the source (heap tags
///   reset to Unit so a later drop will not double-free)
/// - other exprs → load from the compiled pointer and store
///
/// When `bind_name` is `Some`, also runs the existing `set_variable_type` update
/// used by Stmt bind arms (`is_annotated` / `is_ambi` gates unchanged).
fn store_expr_into_dest<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    dest_ptr: PointerValue<'ctx>,
    dest_name: &str,
    expr: &Spanned<ast::Expr>,
    bind_name: Option<&str>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<(), SprsError> {
    let val_ptr = self_compiler.compile_owned_expr(expr, module, "match_bind_owned")?;
    let drop_fn = self_compiler.get_runtime_fn(module, "__drop")?;
    builder_helper::drop_var(self_compiler, dest_ptr, drop_fn, dest_name);
    let new_val = self_compiler
        .builder
        .build_load(self_compiler.runtime_value_type, val_ptr, "match_bind_load")
        .unwrap();
    self_compiler
        .builder
        .build_store(dest_ptr, new_val)
        .unwrap();
    if let Some(bind_name) = bind_name {
        let target = self_compiler
            .get_variables(bind_name)
            .ok_or_else(|| format!("Undefined variable: {}", bind_name))?;
        let rhs_ty = self_compiler.infer_type(expr);
        if !target.is_annotated || target.is_ambi {
            self_compiler.set_variable_type(bind_name, rhs_ty);
        }
    }
    Ok(())
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
    let is_atom_static = matches!(scrut_ty, Type::AtomVal | Type::ClosedLabelSet(_))
        || matches!(&scrut_ty, Type::App(name, _) if name == "Atom");
    let is_label_static = matches!(scrut_ty, Type::Label)
        || matches!(&scrut_ty, Type::App(name, _) if name == "Label");
    validate_match_patterns(
        self_compiler,
        arms.iter().map(|arm| (&arm.pat, arm.span)),
        is_atom_static,
    )?;
    check_closed_label_set_match_exhaustiveness(
        self_compiler,
        &scrut_ty,
        arms.iter().map(|arm| (&arm.pat, arm.span)),
    )?;

    for arm in arms {
        match (&bind, &arm.body) {
            (Some(_), ast::MatchArmBody::ExprBreak(_)) | (None, ast::MatchArmBody::Block(_)) => {}
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
        let is_wildcard = matches!(&arm.pat, ast::MatchPat::Wildcard);
        let mismatch_bb = if is_wildcard {
            None
        } else {
            Some(
                self_compiler
                    .context
                    .append_basic_block(parent_fn, "match_next"),
            )
        };

        // The check for this arm starts where the previous one left off.
        if let Some(prev) = next_bb {
            self_compiler.builder.position_at_end(prev);
        }

        if is_wildcard {
            self_compiler
                .builder
                .build_unconditional_branch(arm_bb)
                .unwrap();
        } else {
            let cond = match_arm_condition(
                self_compiler,
                &scrut_ty,
                &arm.pat,
                tag_val,
                data_val,
                is_atom_static,
                is_label_static,
                module,
            )?;
            self_compiler
                .builder
                .build_conditional_branch(cond, arm_bb, mismatch_bb.unwrap())
                .unwrap();
        }

        // Arm body. The payload binder lives in its own scope so it is
        // dropped when the arm ends (unless moved into the bind variable).
        self_compiler.builder.position_at_end(arm_bb);
        self_compiler.enter_scope();
        bind_label_payload(self_compiler, &arm.pat, data_val, module)?;
        match &arm.body {
            ast::MatchArmBody::ExprBreak(expr) => {
                let bind_name = bind.as_deref().ok_or_else(|| SprsError::Internal {
                    message: "ExprBreak arm without bind".to_string(),
                    location: None,
                })?;
                let target = self_compiler
                    .get_variables(bind_name)
                    .ok_or_else(|| format!("Undefined variable: {}", bind_name))?;
                let target_ptr = target.value.into_pointer_value();
                store_expr_into_dest(
                    self_compiler,
                    target_ptr,
                    bind_name,
                    expr,
                    Some(bind_name),
                    module,
                )?;
                // Drop the arm scope before branching: exit_scope skips drops
                // once a terminator is present, which would leak unused payload clones.
                self_compiler.exit_scope(module)?;
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
                    // Drop the arm scope before branching (see ExprBreak above).
                    self_compiler.exit_scope(module)?;
                    self_compiler
                        .builder
                        .build_unconditional_branch(merge_bb)
                        .unwrap();
                } else {
                    // Already terminated (e.g. return): exit_scope skips drops.
                    self_compiler.exit_scope(module)?;
                }
            }
        }
        next_bb = mismatch_bb;
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

/// `match` expression: `var r = match e { case PAT => expr ... };`.
///
/// Arms evaluate an expression (no `break`); the whole match produces a value
/// stored into a result alloca so the caller receives a pointer like every
/// other expression. Static-type pruning and `case _` behave as in
/// `create_match_stmt`. No arm matches → `__panic("Match failed")`.
pub fn create_match_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    scrutinee: &Spanned<ast::Expr>,
    arms: &Vec<ast::ExprMatchArm>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    // --- Semantic validation (SEM-017) ---
    let scrut_ty = self_compiler.infer_type(scrutinee);
    let is_atom_static = matches!(scrut_ty, Type::AtomVal | Type::ClosedLabelSet(_))
        || matches!(&scrut_ty, Type::App(name, _) if name == "Atom");
    let is_label_static = matches!(scrut_ty, Type::Label)
        || matches!(&scrut_ty, Type::App(name, _) if name == "Label");
    validate_match_patterns(
        self_compiler,
        arms.iter().map(|arm| (&arm.pat, arm.span)),
        is_atom_static,
    )?;
    check_closed_label_set_match_exhaustiveness(
        self_compiler,
        &scrut_ty,
        arms.iter().map(|arm| (&arm.pat, arm.span)),
    )?;

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

    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "match_expr_merge");

    // Result alloca, initialized to Unit like the statement bind variable;
    // every arm stores into it and the caller receives the pointer.
    let result_ptr = create_entry_block_alloca(self_compiler, "match_expr_result")?;
    let unit_expr = Spanned::new(ast::Expr::Unit(), Span::DUMMY);
    let init_val = self_compiler
        .compile_expr(&unit_expr, module)?
        .into_pointer_value();
    let init_loaded = self_compiler
        .builder
        .build_load(
            self_compiler.runtime_value_type,
            init_val,
            "match_expr_result_init",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(result_ptr, init_loaded)
        .unwrap();

    let mut next_bb: Option<inkwell::basic_block::BasicBlock<'ctx>> = None;

    for arm in arms {
        let arm_bb = self_compiler
            .context
            .append_basic_block(parent_fn, "match_expr_arm");
        let is_wildcard = matches!(&arm.pat, ast::MatchPat::Wildcard);
        let mismatch_bb = if is_wildcard {
            None
        } else {
            Some(
                self_compiler
                    .context
                    .append_basic_block(parent_fn, "match_expr_next"),
            )
        };

        if let Some(prev) = next_bb {
            self_compiler.builder.position_at_end(prev);
        }

        if is_wildcard {
            self_compiler
                .builder
                .build_unconditional_branch(arm_bb)
                .unwrap();
        } else {
            let cond = match_arm_condition(
                self_compiler,
                &scrut_ty,
                &arm.pat,
                tag_val,
                data_val,
                is_atom_static,
                is_label_static,
                module,
            )?;
            self_compiler
                .builder
                .build_conditional_branch(cond, arm_bb, mismatch_bb.unwrap())
                .unwrap();
        }

        // Arm body: optional payload binder, then the value expression stored
        // into the result alloca with the same clone/move rules as assignment.
        self_compiler.builder.position_at_end(arm_bb);
        self_compiler.enter_scope();
        bind_label_payload(self_compiler, &arm.pat, data_val, module)?;
        store_expr_into_dest(
            self_compiler,
            result_ptr,
            "match_expr_result",
            &arm.value,
            None,
            module,
        )?;
        if self_compiler
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            // Drop the arm scope before branching: exit_scope skips drops
            // once a terminator is present, which would leak unused payload clones.
            self_compiler.exit_scope(module)?;
            self_compiler
                .builder
                .build_unconditional_branch(merge_bb)
                .unwrap();
        } else {
            // Already terminated: exit_scope skips drops; keeps the scope balanced.
            self_compiler.exit_scope(module)?;
        }
        next_bb = mismatch_bb;
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
    Ok(result_ptr.into())
}

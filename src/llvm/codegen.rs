use crate::front::error::{ErrorCategory, ErrorCode, SprsError};
use crate::front::hir;
use crate::front::label_name::LabelName;
use crate::front::span::Span;
use crate::front::type_helper::{Type, list_element, ptr_element};
use crate::llvm::builder_helper;
use crate::llvm::builder_helper::BuilderExt;
use crate::llvm::builder_helper::Comparison;
use crate::llvm::builder_helper::ContextExt;
use crate::llvm::builder_helper::EqNeq;
use crate::llvm::builder_helper::UpDown;
use crate::llvm::compiler::{Compiler, Tag};
use crate::llvm::value::{build_label_is_error, create_atom, create_label};
use crate::naming;
use inkwell::AddressSpace;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue, ValueKind};

impl<'ctx> Compiler<'ctx> {
    pub fn compile_fn(
        &mut self,
        func: &hir::Function,
        module: &Module<'ctx>,
    ) -> Result<FunctionValue<'ctx>, SprsError> {
        let _arg_types: Vec<BasicMetadataTypeEnum> = (0..func.params.len())
            .map(|_| self.context.ptr_type(AddressSpace::default()).into())
            .collect();

        let func_name = if func.name == "main" {
            naming::INTERNAL_MAIN_FN
        } else {
            &func.name
        };

        let fn_val = module
            .get_function(func_name)
            .ok_or_else(|| SprsError::Internal {
                message: format!("Function {} not declared", func_name),
                location: None,
            })?;

        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);
        self.function_signatures = Some(fn_val);

        self.enter_scope();
        self.attachments.clear();

        for (idx, param) in func.params.iter().enumerate() {
            let arg_val = fn_val.get_nth_param(idx as u32).unwrap();
            // Params are declared as pointers to SprsValue (see declare_fn_prototype).
            let arg_ptr = arg_val.into_pointer_value();

            let alloca = self
                .builder
                .build_alloca(self.runtime_value_type, &param.name)
                .unwrap();
            let loaded = self
                .builder
                .build_load(self.runtime_value_type, arg_ptr, &param.name)
                .unwrap();
            self.builder
                .build_store(alloca, loaded)
                .map_err(|compile_error| SprsError::Internal {
                    message: compile_error.to_string(),
                    location: None,
                })?;
            self.add_variable(param.name.clone(), alloca.into());
        }

        self.compile_block(&func.body, module)?;
        let current_block = self.builder.get_insert_block().unwrap();
        if current_block.get_terminator().is_none() {
            // Body's compile_block already exited its scopes; drop the remaining arg scope here.
            self.emit_drop_for_attachments(module)?;
            self.exit_scope(module)?;
            builder_helper::create_dummy_for_no_return(self)?;
        } else {
            self.scopes.pop();
        }
        self.attachments.clear();

        if fn_val.verify(true) {
            Ok(fn_val)
        } else {
            unsafe {
                fn_val.delete();
            }
            Err(SprsError::Internal {
                message: "Invalid generated function".to_string(),
                location: None,
            })
        }
    }

    pub(crate) fn compile_owned_expr(
        &mut self,
        expr: &hir::Expr,
        module: &Module<'ctx>,
        temp_name: &str,
    ) -> Result<PointerValue<'ctx>, SprsError> {
        if matches!(&expr.kind, hir::ExprKind::Deref(_)) {
            return Ok(self.compile_expr(expr, module)?.into_pointer_value());
        }
        let compiled = self.compile_expr(expr, module)?.into_pointer_value();
        let owned_name = match &expr.kind {
            hir::ExprKind::Var(name) | hir::ExprKind::Assign(name, _) => Some(name.clone()),
            _ => None,
        };
        let Some(name) = owned_name else {
            return Ok(compiled);
        };
        let Some(binding) = self.get_variables(&name) else {
            return Ok(compiled);
        };
        let val_ptr = binding.value.into_pointer_value();
        let copied = builder_helper::var_load_at_init_variable(self, val_ptr, temp_name)?;
        builder_helper::move_variable(self, &val_ptr.into(), &name);
        Ok(copied)
    }

    pub(crate) fn compile_deref_place(
        &mut self,
        pointer: &hir::Expr,
        module: &Module<'ctx>,
    ) -> Result<PointerValue<'ctx>, SprsError> {
        crate::llvm::storage::compile_storage_place(self, pointer, module)
    }

    fn pointee_type_of(pointer: &hir::Expr) -> Result<Type, SprsError> {
        ptr_element(&pointer.ty)
            .cloned()
            .ok_or_else(|| SprsError::Internal {
                message: format!("expected Ptr(T), got {}", pointer.ty),
                location: None,
            })
    }

    /// Process a `return` statement: type-check the expression, convert it
    /// to the function's return type, emit drops, and build the `ret` instr.
    fn compile_return(
        &mut self,
        expr_opt: &Option<hir::Expr>,
        module: &Module<'ctx>,
    ) -> Result<(), SprsError> {
        let ret_val = if let Some(expr) = expr_opt {
            let ptr = self.compile_owned_expr(expr, module, "ret_owned")?;

            let current_fn = self.function_signatures.unwrap();
            let return_type = current_fn.get_type().get_return_type();
            self.convert_return_value(return_type, ptr)?
        } else {
            None
        };

        self.emit_drop_for_return(module)?;

        if let Some(val) = ret_val {
            self.builder.build_return(Some(&val)).unwrap();
        } else {
            builder_helper::create_dummy_for_no_return(self)?;
        }
        Ok(())
    }

    /// Convert the runtime value at `ptr` to the LLVM return type.
    fn convert_return_value(
        &mut self,
        return_type: Option<BasicTypeEnum<'ctx>>,
        ptr: PointerValue<'ctx>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, SprsError> {
        if let Some(ret_ty) = return_type {
            if ret_ty == self.runtime_value_type.into() {
                let val = self
                    .builder
                    .build_load(self.runtime_value_type, ptr, "return_load")
                    .unwrap();
                Ok(Some(val))
            } else {
                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, ptr, 1, "data_ptr")
                    .unwrap();
                let data_val = self
                    .builder
                    .build_load(self.context.i64_type(), data_ptr, "data_load")
                    .unwrap()
                    .into_int_value();

                let casted_val: BasicValueEnum = if ret_ty.is_int_type() {
                    let int_type = ret_ty.into_int_type();
                    if int_type.get_bit_width() < 64 {
                        self.builder
                            .build_int_truncate(data_val, int_type, "truncated")
                            .unwrap()
                            .into()
                    } else {
                        data_val.into()
                    }
                } else if ret_ty.is_float_type() {
                    let float_type = ret_ty.into_float_type();
                    let f64_val = self
                        .builder
                        .build_bit_cast(data_val, self.context.f64_type(), "casted_float")
                        .unwrap()
                        .into_float_value();
                    if float_type.get_bit_width() == 32 {
                        self.builder
                            .build_float_trunc(f64_val, float_type, "truncated_float")
                            .unwrap()
                            .into()
                    } else {
                        f64_val.into()
                    }
                } else if ret_ty.is_pointer_type() {
                    let ptr_type = ret_ty.into_pointer_type();
                    self.builder
                        .build_int_to_ptr(data_val, ptr_type, "int_to_ptr")
                        .unwrap()
                        .into()
                } else {
                    return Err(SprsError::Internal {
                        message: "Unsupported return type conversion".to_string(),
                        location: None,
                    });
                };
                Ok(Some(casted_val))
            }
        } else {
            Ok(None)
        }
    }

    pub(crate) fn compile_block(
        &mut self,
        stmts: &Vec<hir::Stmt>,
        module: &Module<'ctx>,
    ) -> Result<(), SprsError> {
        self.enter_scope(); // New scope for the block

        for stmt in stmts {
            if self
                .builder
                .get_insert_block()
                .unwrap()
                .get_terminator()
                .is_some()
            {
                break;
            }

            match &stmt.kind {
                hir::StmtKind::Var {
                    name,
                    binding_ty: _,
                    is_ambi: _,
                    is_annotated: _,
                    init,
                } => {
                    let init_val = self.compile_owned_expr(init, module, name)?;
                    self.add_variable(name.clone(), init_val.into());
                }
                hir::StmtKind::Return(expr_opt) => {
                    self.compile_return(expr_opt, module)?;
                }
                hir::StmtKind::If {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    builder_helper::create_if_condition(self, cond, then_blk, else_blk, module)?;
                }
                hir::StmtKind::While { cond, body } => {
                    builder_helper::create_while_condition(self, cond, body, module)?;
                }
                hir::StmtKind::Unsafe { body, .. } => {
                    // Always restore depth, including when compile_block returns Err.
                    self.compile_block(body, module)?;
                }
                hir::StmtKind::Defer { expr, .. } => {
                    // Queue only; exit_scope / emit_drop_for_return execute LIFO later.
                    if let Some(scope) = self.scopes.last_mut() {
                        scope.deferred.push(expr.clone());
                    }
                }
                hir::StmtKind::Match {
                    scrutinee,
                    bind,
                    arms,
                    ..
                } => {
                    builder_helper::create_match_stmt(self, scrutinee, bind, arms, module)?;
                }
                hir::StmtKind::Expr(expr) => {
                    self.compile_expr(expr, module)?;
                }
                hir::StmtKind::Assign { name, rhs } => {
                    self.emit_named_assign(name, rhs, module, stmt.span)?;
                }
                hir::StmtKind::IndexAssign {
                    collection,
                    index,
                    expr,
                } => {
                    let coll_ty = collection.ty.clone();
                    let is_list = list_element(&coll_ty).is_some();
                    let buf_ptr = self.compile_expr(collection, module)?.into_pointer_value();
                    let buf_data_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, buf_ptr, 1, "ia_buf_data_ptr")
                        .unwrap();
                    let buf_handle = self
                        .builder
                        .build_load(self.context.i64_type(), buf_data_ptr, "ia_buf_handle")
                        .unwrap()
                        .into_int_value();

                    let idx_ptr = self.compile_expr(index, module)?.into_pointer_value();
                    let idx_data_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, idx_ptr, 1, "ia_idx_data_ptr")
                        .unwrap();
                    let idx_val = self
                        .builder
                        .build_load(self.context.i64_type(), idx_data_ptr, "ia_idx")
                        .unwrap()
                        .into_int_value();

                    let v_ptr = self.compile_expr(expr, module)?.into_pointer_value();
                    if is_list {
                        let v_tag_ptr = self
                            .builder
                            .build_struct_gep(self.runtime_value_type, v_ptr, 0, "ia_v_tag_ptr")
                            .unwrap();
                        let v_tag = self
                            .builder
                            .build_load(self.context.i32_type(), v_tag_ptr, "ia_v_tag")
                            .unwrap()
                            .into_int_value();
                        let v_data_ptr = self
                            .builder
                            .build_struct_gep(self.runtime_value_type, v_ptr, 1, "ia_v_data_ptr")
                            .unwrap();
                        let v_val = self
                            .builder
                            .build_load(self.context.i64_type(), v_data_ptr, "ia_v")
                            .unwrap()
                            .into_int_value();
                        let set_fn = self.get_runtime_fn(module, "__list_set")?;
                        self.builder
                            .build_call(
                                set_fn,
                                &[
                                    buf_handle.into(),
                                    idx_val.into(),
                                    v_tag.into(),
                                    v_val.into(),
                                ],
                                "list_set_call",
                            )
                            .unwrap();
                    } else {
                        let v_data_ptr = self
                            .builder
                            .build_struct_gep(self.runtime_value_type, v_ptr, 1, "ia_v_data_ptr")
                            .unwrap();
                        let v_val = self
                            .builder
                            .build_load(self.context.i64_type(), v_data_ptr, "ia_v")
                            .unwrap()
                            .into_int_value();

                        let set_fn = self.get_runtime_fn(module, "__buffer_set")?;
                        self.builder
                            .build_call(
                                set_fn,
                                &[buf_handle.into(), idx_val.into(), v_val.into()],
                                "buffer_set_call",
                            )
                            .unwrap();
                    }
                }
                hir::StmtKind::DerefAssign { pointer, expr } => {
                    let dest = self.compile_deref_place(pointer, module)?;
                    let val_ptr = self.compile_owned_expr(expr, module, "deref_assign_owned")?;
                    let pointee = Self::pointee_type_of(pointer)?;
                    crate::llvm::storage::store_runtime_to_storage(
                        self, module, dest, &pointee, val_ptr, true,
                    )?;
                }
            }
        }

        self.exit_scope(module)?;

        Ok(())
    }

    fn emit_named_assign(
        &mut self,
        name: &str,
        rhs: &hir::Expr,
        module: &Module<'ctx>,
        span: Span,
    ) -> Result<PointerValue<'ctx>, SprsError> {
        let target = self
            .get_variables(name)
            .ok_or_else(|| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 2,
                },
                location: self.location(span),
                message: format!("Undefined variable: {}", name),
                help: None,
            })?;
        let target_ptr = target.value.into_pointer_value();

        // Self-assign is a no-op: drop-then-load on the same binding
        // would destroy the value.
        if let hir::ExprKind::Var(src_val_name) = &rhs.kind {
            if src_val_name == name {
                return Ok(target_ptr);
            }
        }

        let val_ptr = self.compile_owned_expr(rhs, module, "assign_owned")?;

        let _target = self
            .get_variables(name)
            .ok_or_else(|| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 2,
                },
                location: self.location(span),
                message: format!("Undefined variable: {}", name),
                help: None,
            })?;

        let drop_fn = self.get_runtime_fn(module, "__drop")?;
        builder_helper::drop_var(self, target_ptr, drop_fn, name);

        let new_val = self
            .builder
            .build_load(self.runtime_value_type, val_ptr, "assign_load")
            .unwrap();
        self.builder.build_store(target_ptr, new_val).unwrap();

        Ok(target_ptr)
    }

    pub(crate) fn compile_expr(
        &mut self,
        expr: &hir::Expr,
        module: &Module<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, SprsError> {
        match &expr.kind {
            hir::ExprKind::Number(number_value) => {
                Ok(builder_helper::create_integer(self, number_value)?)
            }
            hir::ExprKind::Float(fp) => Ok(builder_helper::create_float(self, *fp)?),
            hir::ExprKind::TypeI8 => builder_helper::create_int8(self),
            hir::ExprKind::TypeU8 => builder_helper::create_uint8(self),
            hir::ExprKind::TypeI16 => builder_helper::create_int16(self),
            hir::ExprKind::TypeU16 => builder_helper::create_uint16(self),
            hir::ExprKind::TypeI32 => builder_helper::create_int32(self),
            hir::ExprKind::TypeU32 => builder_helper::create_uint32(self),
            hir::ExprKind::TypeI64 => builder_helper::create_int64(self),
            hir::ExprKind::TypeU64 => builder_helper::create_uint64(self),
            hir::ExprKind::TypeUsize => builder_helper::create_uint64(self),
            hir::ExprKind::TypeF16 => builder_helper::create_float16(self),
            hir::ExprKind::TypeF32 => builder_helper::create_float32(self),
            hir::ExprKind::TypeF64 => builder_helper::create_float64(self),
            hir::ExprKind::Str(str) => Ok(builder_helper::create_string(self, str, module)?),
            hir::ExprKind::Bool(boolean) => Ok(builder_helper::create_bool(self, boolean)?),
            hir::ExprKind::Assign(name, rhs) => {
                Ok(self.emit_named_assign(name, rhs, module, expr.span)?.into())
            }
            hir::ExprKind::AtomRef(ident) => Ok(create_atom(
                self,
                &LabelName::Static(ident.clone()),
                module,
            )?),
            hir::ExprKind::Var(ident) => {
                if let Some(binding) = self.get_variables(ident) {
                    Ok(binding.value)
                } else {
                    Err(SprsError::Semantic {
                        code: ErrorCode {
                            category: ErrorCategory::Semantic,
                            number: 2,
                        },
                        location: self.location(expr.span),
                        message: format!("Undefined variable: {}", ident),
                        help: None,
                    })
                }
            }
            hir::ExprKind::Call { callee, args } => {
                let ident = self.resolve_callable_backend_name(callee, module)?;
                Ok(builder_helper::create_call_expr(
                    self, &ident, args, module,
                )?)
            }
            hir::ExprKind::Macro(ident, args) => match ident.as_str() {
                "println" => Ok(builder_helper::call_builtin_macro_println(
                    self, args, module,
                )?),
                "list_push" => Ok(builder_helper::call_builtin_macro_list_push(
                    self, args, module,
                )?),
                "buf_len" => Ok(builder_helper::call_builtin_macro_buf_len(
                    self, args, module,
                )?),
                "buf_get" => Ok(builder_helper::call_builtin_macro_buf_get(
                    self, args, module,
                )?),
                "buf_set" => Ok(builder_helper::call_builtin_macro_buf_set(
                    self, args, module,
                )?),
                "clone" => Ok(builder_helper::call_builtin_macro_clone(
                    self, args, module,
                )?),
                "move" => Ok(builder_helper::call_builtin_macro_move(self, args, module)?),
                "raw" => Ok(builder_helper::call_builtin_macro_raw(self, args, module)?),
                "free" => Ok(builder_helper::call_builtin_macro_free(self, args, module)?),
                "cast" => Ok(builder_helper::call_builtin_macro_cast(self, args, module)?),
                "fcast" => Ok(builder_helper::call_builtin_macro_fcast(
                    self, args, module,
                )?),
                "lshift" => Ok(builder_helper::call_builtin_macro_lshift(
                    self, args, module,
                )?),
                "rshift" => Ok(builder_helper::call_builtin_macro_rshift(
                    self, args, module,
                )?),
                "not" => Ok(builder_helper::call_builtin_macro_not(self, args, module)?),
                "is_error" => Ok(builder_helper::call_builtin_macro_is_error(
                    self, args, module,
                )?),
                "error_message" => Ok(builder_helper::call_builtin_macro_error_message(
                    self, args, module,
                )?),
                "attach" => Ok(builder_helper::call_builtin_macro_attach(
                    self, args, module,
                )?),
                "label_is" => Ok(builder_helper::call_builtin_macro_label_is(
                    self, args, module,
                )?),
                "label_payload" => Ok(builder_helper::call_builtin_macro_label_payload(
                    self, args, module,
                )?),
                "label_name" => Ok(builder_helper::call_builtin_macro_label_name(
                    self, args, module,
                )?),
                "error" => Ok(builder_helper::call_builtin_macro_error(
                    self, args, module,
                )?),
                "init" => Ok(builder_helper::call_builtin_macro_init(self, args, module)?),
                "ref" => Ok(builder_helper::call_builtin_macro_ref(self, args, module)?),
                "take" => Ok(builder_helper::call_builtin_macro_take(self, args, module)?),
                _ => Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 3,
                    },
                    location: self.location(expr.span),
                    message: format!("Unknown macro: {}", ident),
                    help: None,
                }),
            },
            hir::ExprKind::FieldAccess {
                receiver,
                struct_ref,
                field_index,
                ..
            } => {
                let struct_name = self.resolve_struct_backend_name(struct_ref)?;
                Ok(builder_helper::create_field_access(
                    self,
                    receiver,
                    *field_index,
                    &struct_name,
                    module,
                )?)
            }
            hir::ExprKind::Add(lhs, rhs) => {
                if ptr_element(&lhs.ty).is_some() {
                    Ok(builder_helper::create_ptr_add_expr(self, lhs, rhs, module)?)
                } else {
                    Ok(builder_helper::create_add_expr(self, lhs, rhs, module)?)
                }
            }
            hir::ExprKind::Mul(lhs, rhs) => {
                Ok(builder_helper::create_mul_expr(self, lhs, rhs, module)?)
            }
            hir::ExprKind::Minus(lhs, rhs) => {
                Ok(builder_helper::create_minus_expr(self, lhs, rhs, module)?)
            }
            hir::ExprKind::Div(lhs, rhs) => {
                Ok(builder_helper::create_div_expr(self, lhs, rhs, module)?)
            }
            hir::ExprKind::Mod(lhs, rhs) => {
                Ok(builder_helper::create_mod_expr(self, lhs, rhs, module)?)
            }
            hir::ExprKind::Increment(expr) => Ok(builder_helper::create_increment_or_decrement(
                self,
                expr,
                UpDown::Up,
                module,
            )?),
            hir::ExprKind::Decrement(expr) => Ok(builder_helper::create_increment_or_decrement(
                self,
                expr,
                UpDown::Down,
                module,
            )?),
            hir::ExprKind::Neg(expr) => {
                let zero = hir::Expr {
                    kind: hir::ExprKind::Number(0),
                    ty: Type::Int,
                    span: Span::DUMMY,
                };
                Ok(builder_helper::create_minus_expr(
                    self, &zero, expr, module,
                )?)
            }
            hir::ExprKind::Deref(pointer) => {
                let dest = self.compile_deref_place(pointer, module)?;
                let pointee = Self::pointee_type_of(pointer)?;
                let loaded = crate::llvm::storage::load_storage_as_runtime(
                    self,
                    module,
                    dest,
                    &pointee,
                    crate::llvm::storage::StorageLoad::Clone,
                )?;
                Ok(loaded.into())
            }
            hir::ExprKind::Eq(lhs, rhs) => Ok(builder_helper::create_eq_or_neq(
                self,
                lhs,
                rhs,
                module,
                EqNeq::Eq,
                |builder, l_val, r_val, name| {
                    Ok(builder
                        .build_int_compare(inkwell::IntPredicate::EQ, l_val, r_val, name)
                        .unwrap())
                },
            )?),
            hir::ExprKind::Neq(lhs, rhs) => Ok(builder_helper::create_eq_or_neq(
                self,
                lhs,
                rhs,
                module,
                EqNeq::Neq,
                |builder, l_val, r_val, name| {
                    Ok(builder
                        .build_int_compare(inkwell::IntPredicate::NE, l_val, r_val, name)
                        .unwrap())
                },
            )?),
            hir::ExprKind::Gt(lhs, rhs) => Ok(builder_helper::create_comparison(
                self,
                lhs,
                rhs,
                module,
                Comparison::Gt,
                |builder, l_val, r_val, name| {
                    Ok(builder
                        .build_int_compare(inkwell::IntPredicate::SGT, l_val, r_val, name)
                        .unwrap())
                },
            )?),
            hir::ExprKind::Lt(lhs, rhs) => Ok(builder_helper::create_comparison(
                self,
                lhs,
                rhs,
                module,
                Comparison::Lt,
                |builder, l_val, r_val, name| {
                    Ok(builder
                        .build_int_compare(inkwell::IntPredicate::SLT, l_val, r_val, name)
                        .unwrap())
                },
            )?),
            hir::ExprKind::Ge(lhs, rhs) => Ok(builder_helper::create_comparison(
                self,
                lhs,
                rhs,
                module,
                Comparison::Ge,
                |builder, l_val, r_val, name| {
                    Ok(builder
                        .build_int_compare(inkwell::IntPredicate::SGE, l_val, r_val, name)
                        .unwrap())
                },
            )?),
            hir::ExprKind::Le(lhs, rhs) => Ok(builder_helper::create_comparison(
                self,
                lhs,
                rhs,
                module,
                Comparison::Le,
                |builder, l_val, r_val, name| {
                    Ok(builder
                        .build_int_compare(inkwell::IntPredicate::SLE, l_val, r_val, name)
                        .unwrap())
                },
            )?),
            hir::ExprKind::Match { scrutinee, arms } => Ok(builder_helper::create_match_expr(
                self, scrutinee, arms, module,
            )?),
            hir::ExprKind::List(elements) => {
                Ok(builder_helper::create_list(self, elements, module)?)
            }
            hir::ExprKind::Index(collection_expr, index_expr) => Ok(builder_helper::create_index(
                self,
                collection_expr,
                index_expr,
                module,
            )?),
            hir::ExprKind::Range(start_expr, end_expr) => Ok(builder_helper::create_range(
                self, start_expr, end_expr, module,
            )?),
            hir::ExprKind::ModuleAccess(module_name, function_name, args) => {
                Ok(builder_helper::create_module_access(
                    self,
                    module_name,
                    function_name,
                    args,
                    module,
                )?)
            }
            hir::ExprKind::Unit() => Ok(builder_helper::create_unit(self)?),
            hir::ExprKind::Atom(name) => Ok(create_atom(self, name, module)?),
            hir::ExprKind::Label(name, payload) => Ok(create_label(self, name, payload, module)?),
            hir::ExprKind::AttachSlot(slot_name) => {
                if let Some(attached) = self.attachments.get(slot_name).copied() {
                    Ok(builder_helper::clone_runtime_value(self, attached, module)?.into())
                } else {
                    Err(SprsError::Semantic {
                        code: ErrorCode {
                            category: ErrorCategory::Semantic,
                            number: 2,
                        },
                        location: self.location(expr.span),
                        message: format!("attach slot '<:{}' used before @attach", slot_name),
                        help: None,
                    })
                }
            }
            hir::ExprKind::StructInit { struct_ref, fields } => {
                let struct_name = self.resolve_struct_backend_name(struct_ref)?;
                Ok(builder_helper::create_struct_init(
                    self,
                    &struct_name,
                    fields,
                    module,
                )?)
            }
            hir::ExprKind::Try(inner_expr) => {
                let inner_ptr = self.compile_owned_expr(inner_expr, module, "try_owned")?;

                // Load the tag and data of the inner result.
                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, inner_ptr, 0, "try_tag_ptr")
                    .unwrap();
                let tag_val = self
                    .builder
                    .build_load(self.context.i32_type(), tag_ptr, "try_tag")
                    .unwrap()
                    .into_int_value();
                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, inner_ptr, 1, "try_data_ptr")
                    .unwrap();
                let data_val = self
                    .builder
                    .build_load(self.context.i64_type(), data_ptr, "try_data")
                    .unwrap()
                    .into_int_value();

                // `?` propagates only labels named "error".
                let is_error = build_label_is_error(self, tag_val, data_val, module)?;

                let current_fn = self.function_signatures.unwrap();
                let propagate_bb = self.context.append_basic_block(current_fn, "try_propagate");
                let continue_bb = self.context.append_basic_block(current_fn, "try_continue");

                let _ = self
                    .builder
                    .build_conditional_branch(is_error, propagate_bb, continue_bb);

                self.builder.position_at_end(propagate_bb);
                self.emit_drop_for_return(module)?;
                let return_type = current_fn.get_type().get_return_type();
                let ret_val = self.convert_return_value(return_type, inner_ptr)?;
                if let Some(val) = ret_val {
                    self.builder.build_return(Some(&val)).unwrap();
                } else {
                    self.builder.build_return(None).unwrap();
                }

                self.builder.position_at_end(continue_bb);
                Ok(inner_ptr.into())
            }
            hir::ExprKind::HeapAlloc(size_expr) => {
                let size_ptr = self.compile_expr(size_expr, module)?.into_pointer_value();
                let size_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, size_ptr, 1, "buf_size_data_ptr")
                    .unwrap();
                let size_val = self
                    .builder
                    .build_load(self.context.i64_type(), size_data_ptr, "buf_size")
                    .unwrap()
                    .into_int_value();

                let buffer_new_fn = self.get_runtime_fn(module, "__buffer_new")?;
                let handle = match self
                    .builder
                    .build_call(buffer_new_fn, &[size_val.into()], "buffer_new_call")
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(val) => val.into_int_value(),
                    ValueKind::Instruction(_) => {
                        return Err(SprsError::Internal {
                            message: "__buffer_new returned void".to_string(),
                            location: None,
                        });
                    }
                };

                let res_ptr = builder_helper::create_entry_block_alloca(self, "heap_alloc_res")?;
                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context.i32_type().const_int(Tag::Buffer as u64, false),
                    )
                    .unwrap();
                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                self.builder.build_store(res_data_ptr, handle).unwrap();

                Ok(res_ptr.into())
            }
            hir::ExprKind::Destroy(inner_expr) => {
                let val_ptr = self.compile_expr(inner_expr, module)?.into_pointer_value();

                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, val_ptr, 0, "destroy_tag_ptr")
                    .unwrap();
                let tag_val = self
                    .builder
                    .build_load(self.context.i32_type(), tag_ptr, "destroy_tag")
                    .unwrap()
                    .into_int_value();
                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, val_ptr, 1, "destroy_data_ptr")
                    .unwrap();
                let data_val = self
                    .builder
                    .build_load(self.context.i64_type(), data_ptr, "destroy_data")
                    .unwrap()
                    .into_int_value();

                let drop_fn = self.get_runtime_fn(module, "__drop")?;
                self.builder
                    .build_call(
                        drop_fn,
                        &[tag_val.into(), data_val.into()],
                        "destroy_drop_call",
                    )
                    .unwrap();

                // Cut ownership so a later auto-drop is a no-op.
                self.builder
                    .build_store(
                        tag_ptr,
                        self.context.i32_type().const_int(Tag::Unit as u64, false),
                    )
                    .unwrap();

                let res_ptr = builder_helper::create_entry_block_alloca(self, "destroy_res_alloc")?;
                self.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "destroy_unit");
                Ok(res_ptr.into())
            }
            hir::ExprKind::Exist(inner_expr) => {
                let val_ptr = self.compile_expr(inner_expr, module)?.into_pointer_value();

                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, val_ptr, 0, "exist_tag_ptr")
                    .unwrap();
                let tag_val = self
                    .builder
                    .build_load(self.context.i32_type(), tag_ptr, "exist_tag")
                    .unwrap()
                    .into_int_value();
                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, val_ptr, 1, "exist_data_ptr")
                    .unwrap();
                let data_val = self
                    .builder
                    .build_load(self.context.i64_type(), data_ptr, "exist_data")
                    .unwrap()
                    .into_int_value();

                let tag_buffer = self.get_tag_from_tag_enum(Tag::Buffer);
                let is_buffer = self.tag_cmp(
                    inkwell::IntPredicate::EQ,
                    tag_val,
                    tag_buffer,
                    "exist_is_buffer",
                );

                let current_fn = self.get_current_function();
                let check_bb = self
                    .context
                    .append_basic_block(current_fn, "exist_check_bb");
                let false_bb = self
                    .context
                    .append_basic_block(current_fn, "exist_false_bb");
                let cont_bb = self.context.append_basic_block(current_fn, "exist_cont_bb");

                let res_ptr = builder_helper::create_entry_block_alloca(self, "exist_res_alloc")?;

                let _ = self
                    .builder
                    .build_conditional_branch(is_buffer, check_bb, false_bb);

                self.builder.position_at_end(check_bb);
                let exist_fn = self.get_runtime_fn(module, "__buffer_exist")?;
                let exist_res = match self
                    .builder
                    .build_call(exist_fn, &[data_val.into()], "buffer_exist_call")
                    .unwrap()
                    .try_as_basic_value()
                {
                    ValueKind::Basic(val) => val.into_int_value(),
                    ValueKind::Instruction(_) => {
                        return Err(SprsError::Internal {
                            message: "__buffer_exist returned void".to_string(),
                            location: None,
                        });
                    }
                };
                let exist_ext = self
                    .builder
                    .build_int_z_extend(exist_res, self.context.i64_type(), "exist_data")
                    .unwrap();
                let check_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "exist_res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        check_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();
                let check_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "exist_res_data_ptr")
                    .unwrap();
                self.builder.build_store(check_data_ptr, exist_ext).unwrap();
                self.builder.build_unconditional_branch(cont_bb).unwrap();

                self.builder.position_at_end(false_bb);
                self.tag_only_runtime_value_store(res_ptr, Tag::Boolean as u64, "exist_false_unit");
                let false_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "exist_res_data_ptr")
                    .unwrap();
                self.builder
                    .build_store(false_data_ptr, self.context.i64_type().const_int(0, false))
                    .unwrap();
                self.builder.build_unconditional_branch(cont_bb).unwrap();

                self.builder.position_at_end(cont_bb);
                Ok(res_ptr.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::hir;
    use crate::front::span::Span;
    use crate::front::type_helper::Type;
    use crate::llvm::compiler::{StoreTag, StoreValue};
    use crate::llvm::value::create_entry_block_alloca;
    use crate::runtime::runtime::{self as sprs_runtime, SprsValue};
    use inkwell::OptimizationLevel;
    use inkwell::context::Context;
    use inkwell::targets::{InitializationConfig, Target};

    fn ptr_i64() -> Type {
        Type::App("Ptr".into(), vec![Type::TypeI64])
    }

    fn ptr_mu_i64() -> Type {
        Type::App(
            "Ptr".into(),
            vec![Type::App("MaybeUninit".into(), vec![Type::TypeI64])],
        )
    }

    fn dummy_span() -> Span {
        Span::DUMMY
    }

    fn expr(kind: hir::ExprKind, ty: Type) -> hir::Expr {
        hir::Expr {
            kind,
            ty,
            span: dummy_span(),
        }
    }

    fn stmt(kind: hir::StmtKind) -> hir::Stmt {
        hir::Stmt {
            kind,
            span: dummy_span(),
        }
    }

    fn var_p() -> hir::Expr {
        expr(hir::ExprKind::Var("p".into()), ptr_i64())
    }

    fn var_mu_p() -> hir::Expr {
        expr(hir::ExprKind::Var("p".into()), ptr_mu_i64())
    }

    fn compile_i64_slots<'ctx>(
        compiler: &mut Compiler<'ctx>,
        context: &'ctx Context,
        slot0: u64,
        slot1: u64,
        ptr_ty: Type,
    ) {
        let i64_ty = context.i64_type();
        let array_ty = i64_ty.array_type(2);
        let cells = compiler.builder.build_alloca(array_ty, "cells").unwrap();
        let zero = i64_ty.const_int(0, false);
        let one = i64_ty.const_int(1, false);
        let slot0_ptr = unsafe {
            compiler
                .builder
                .build_in_bounds_gep(array_ty, cells, &[zero, zero], "slot0")
        }
        .unwrap();
        let slot1_ptr = unsafe {
            compiler
                .builder
                .build_in_bounds_gep(array_ty, cells, &[zero, one], "slot1")
        }
        .unwrap();
        compiler
            .builder
            .build_store(slot0_ptr, i64_ty.const_int(slot0, true))
            .unwrap();
        compiler
            .builder
            .build_store(slot1_ptr, i64_ty.const_int(slot1, false))
            .unwrap();
        let p = create_entry_block_alloca(compiler, "p").expect("p");
        let addr = compiler
            .builder
            .build_ptr_to_int(slot0_ptr, i64_ty, "pointee_addr")
            .unwrap();
        compiler.build_runtime_value_store(
            p,
            StoreTag::Int(Tag::RawPtr as u64),
            StoreValue::Int(addr),
            "ptr",
        );
        let _ = ptr_ty;
        compiler.add_variable("p".into(), p.into());
    }

    fn compile_deref_fixture<'ctx>(
        compiler: &mut Compiler<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        body: Vec<hir::Stmt>,
        name: &str,
        slot0: u64,
        slot1: u64,
        ptr_ty: Type,
    ) {
        let func = hir::Function {
            name: name.to_string(),
            params: Vec::new(),
            body,
            ret_ty: Some(Type::Int),
            is_public: true,
            type_params: Vec::new(),
            when_rules: Vec::new(),
            span: dummy_span(),
        };
        compiler.declare_fn_prototype(&func, module);
        let fn_val = module.get_function(name).expect("prototype");
        let entry = context.append_basic_block(fn_val, "entry");
        compiler.builder.position_at_end(entry);
        compiler.function_signatures = Some(fn_val);
        compiler.enter_scope();

        compile_i64_slots(compiler, context, slot0, slot1, ptr_ty);
        let _ = compiler
            .get_runtime_fn(module, "__drop")
            .expect("declare drop");
        let _ = compiler
            .get_runtime_fn(module, "__clone")
            .expect("declare clone");
        let _ = compiler
            .get_runtime_fn(module, "__panic")
            .expect("declare panic");
        let _ = compiler
            .get_runtime_fn(module, "__struct_new")
            .expect("declare struct_new");
        let _ = compiler
            .get_runtime_fn(module, "__struct_borrow")
            .expect("declare struct_borrow");
        let _ = compiler
            .get_runtime_fn(module, "__struct_track_value")
            .expect("declare track");
        let _ = compiler
            .get_runtime_fn(module, "__struct_forget_owned")
            .expect("declare forget");

        compiler
            .compile_block(&func.body, module)
            .expect("compile body");
        if compiler
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            compiler.exit_scope(module).expect("exit");
        } else if !compiler.scopes.is_empty() {
            compiler.scopes.pop();
        }
        assert!(fn_val.verify(true), "invalid generated function {name}");
    }

    fn map_runtime(engine: &inkwell::execution_engine::ExecutionEngine, module: &Module) {
        if let Some(drop_fn) = module.get_function("__drop") {
            engine.add_global_mapping(&drop_fn, sprs_runtime::__drop as *const () as usize);
        }
        if let Some(clone_fn) = module.get_function("__clone") {
            engine.add_global_mapping(&clone_fn, sprs_runtime::__clone as *const () as usize);
        }
        if let Some(panic_fn) = module.get_function("__panic") {
            engine.add_global_mapping(&panic_fn, sprs_runtime::__panic as *const () as usize);
        }
        if let Some(f) = module.get_function("__struct_new") {
            engine.add_global_mapping(&f, sprs_runtime::__struct_new as *const () as usize);
        }
        if let Some(f) = module.get_function("__struct_borrow") {
            engine.add_global_mapping(&f, sprs_runtime::__struct_borrow as *const () as usize);
        }
        if let Some(f) = module.get_function("__struct_track_value") {
            engine.add_global_mapping(&f, sprs_runtime::__struct_track_value as *const () as usize);
        }
        if let Some(f) = module.get_function("__struct_forget_owned") {
            engine.add_global_mapping(
                &f,
                sprs_runtime::__struct_forget_owned as *const () as usize,
            );
        }
        if let Some(f) = module.get_function("__string_new") {
            engine.add_global_mapping(&f, sprs_runtime::__string_new as *const () as usize);
        }
        if let Some(f) = module.get_function("__string_from_cstr") {
            engine.add_global_mapping(&f, sprs_runtime::__string_from_cstr as *const () as usize);
        }
        if let Some(f) = module.get_function("__error_label_from_str") {
            engine.add_global_mapping(
                &f,
                sprs_runtime::__error_label_from_str as *const () as usize,
            );
        }
    }

    fn add_one(ptr_ty: Type) -> hir::Expr {
        expr(
            hir::ExprKind::Add(
                Box::new(expr(hir::ExprKind::Var("p".into()), ptr_ty.clone())),
                Box::new(expr(hir::ExprKind::Number(1), Type::TypeUsize)),
            ),
            ptr_ty,
        )
    }

    fn deref_p() -> hir::Expr {
        expr(hir::ExprKind::Deref(Box::new(var_p())), Type::TypeI64)
    }

    fn mu_deref_p() -> hir::Expr {
        expr(
            hir::ExprKind::Deref(Box::new(var_mu_p())),
            Type::App("MaybeUninit".into(), vec![Type::TypeI64]),
        )
    }

    fn jit_i64(module: &Module, name: &str) -> SprsValue {
        Target::initialize_native(&InitializationConfig::default()).expect("native target");
        let engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .expect("jit");
        map_runtime(&engine, module);
        type TestFn = unsafe extern "C" fn() -> SprsValue;
        let f = unsafe { engine.get_function::<TestFn>(name) }.expect("lookup");
        unsafe { f.call() }
    }

    #[test]
    fn deref_place_reads_and_replaces_storage() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "deref_test.sprs".into());
        let module = context.create_module("deref_place_test");
        let body = vec![
            stmt(hir::StmtKind::DerefAssign {
                pointer: var_p(),
                expr: expr(hir::ExprKind::Number(42), Type::TypeI64),
            }),
            stmt(hir::StmtKind::Return(Some(deref_p()))),
        ];
        compile_deref_fixture(
            &mut compiler,
            &context,
            &module,
            body,
            "deref_place",
            41,
            0,
            ptr_i64(),
        );
        let result = jit_i64(&module, "deref_place");
        assert_eq!(result.tag, Tag::Integer as i32);
        assert_eq!(result.data, 42);
    }

    #[test]
    fn deref_self_replace_clones_before_store() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "deref_self.sprs".into());
        let module = context.create_module("deref_self_test");
        let body = vec![
            stmt(hir::StmtKind::DerefAssign {
                pointer: var_p(),
                expr: deref_p(),
            }),
            stmt(hir::StmtKind::Return(Some(deref_p()))),
        ];
        compile_deref_fixture(
            &mut compiler,
            &context,
            &module,
            body,
            "deref_self",
            41,
            0,
            ptr_i64(),
        );
        let ir = module.print_to_string().to_string();
        assert!(
            ir.contains("@__clone"),
            "expected __clone for *p = *p\n{ir}"
        );
        let result = jit_i64(&module, "deref_self");
        assert_eq!(result.tag, Tag::Integer as i32);
        assert_eq!(result.data, 41);
    }

    #[test]
    fn ptr_add_uses_storage_rep_stride() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "ptr_add.sprs".into());
        let module = context.create_module("ptr_add_test");
        let body = vec![stmt(hir::StmtKind::Return(Some(expr(
            hir::ExprKind::Deref(Box::new(add_one(ptr_i64()))),
            Type::TypeI64,
        ))))];
        compile_deref_fixture(
            &mut compiler,
            &context,
            &module,
            body,
            "ptr_add",
            41,
            42,
            ptr_i64(),
        );
        let ir = module.print_to_string().to_string();
        assert!(ir.contains("llvm.umul.with.overflow"), "{ir}");
        assert!(ir.contains("llvm.uadd.with.overflow"), "{ir}");
        assert!(ir.contains("Pointer arithmetic overflow"), "{ir}");
        assert!(
            ir.contains("umul.with.overflow.i64(i64 8,")
                || ir.contains("i64 8, i64 %ptr_add_offset")
                || ir.contains("i64 %ptr_add_offset, i64 8"),
            "expected i64 stride 8\n{ir}"
        );
        let result = jit_i64(&module, "ptr_add");
        assert_eq!(result.tag, Tag::Integer as i32);
        assert_eq!(result.data, 42);
    }

    fn stride_ir_for(pointee: Type, expected: u64) {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "stride.sprs".into());
        let module = context.create_module("stride_test");
        let ptr_ty = Type::App("Ptr".into(), vec![pointee]);
        let body = vec![stmt(hir::StmtKind::Return(Some(add_one(ptr_ty.clone()))))];
        let func = hir::Function {
            name: "stride".into(),
            params: Vec::new(),
            body,
            ret_ty: Some(ptr_ty.clone()),
            is_public: true,
            type_params: Vec::new(),
            when_rules: Vec::new(),
            span: dummy_span(),
        };
        compiler.declare_fn_prototype(&func, &module);
        let fn_val = module.get_function("stride").expect("prototype");
        let entry = context.append_basic_block(fn_val, "entry");
        compiler.builder.position_at_end(entry);
        compiler.function_signatures = Some(fn_val);
        compiler.enter_scope();
        let p = create_entry_block_alloca(&mut compiler, "p").expect("p");
        compiler.build_runtime_value_store(
            p,
            StoreTag::Int(Tag::RawPtr as u64),
            StoreValue::Int(context.i64_type().const_int(0, false)),
            "ptr",
        );
        compiler.add_variable("p".into(), p.into());
        let _ = compiler.get_runtime_fn(&module, "__panic").expect("panic");
        compiler
            .compile_block(&func.body, &module)
            .expect("compile");
        let ir = module.print_to_string().to_string();
        let needle = format!("i64 {expected}");
        assert!(
            ir.contains(&format!("overflow.i64(i64 {expected},"))
                || ir.contains(&format!(", i64 {expected})"))
                || ir.contains(&needle),
            "expected stride {expected}\n{ir}"
        );
    }

    #[test]
    fn pointer_strides_match_storage_rep() {
        stride_ir_for(Type::TypeI8, 1);
        stride_ir_for(Type::TypeI32, 4);
        stride_ir_for(Type::TypeI64, 8);
        stride_ir_for(Type::TypeUsize, 8);
        stride_ir_for(Type::App("MaybeUninit".into(), vec![Type::TypeI64]), 8);
    }

    #[test]
    fn padded_struct_pointer_stride_uses_abi_size() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "padded.sprs".into());
        let module = context.create_module("padded_stride");
        compiler
            .register_struct(
                "Padded".into(),
                vec![
                    crate::front::ast::StructField {
                        ident: "a".into(),
                        ty: Some(Type::TypeI8),
                        default_value: None,
                        span: dummy_span(),
                    },
                    crate::front::ast::StructField {
                        ident: "b".into(),
                        ty: Some(Type::TypeI64),
                        default_value: None,
                        span: dummy_span(),
                    },
                ],
            )
            .unwrap();
        let padded = Type::Struct("Padded".into());
        let layout = compiler.storage_layout(&padded).unwrap();
        assert_eq!(layout.size, 16);
        assert_eq!(layout.align, 8);
        assert_ne!(layout.size, 1 + 8, "must not use field-size sum");
        let ptr_ty = Type::App("Ptr".into(), vec![padded]);
        let body = vec![stmt(hir::StmtKind::Return(Some(add_one(ptr_ty.clone()))))];
        let func = hir::Function {
            name: "stride".into(),
            params: Vec::new(),
            body,
            ret_ty: Some(ptr_ty.clone()),
            is_public: true,
            type_params: Vec::new(),
            when_rules: Vec::new(),
            span: dummy_span(),
        };
        compiler.declare_fn_prototype(&func, &module);
        let fn_val = module.get_function("stride").expect("prototype");
        let entry = context.append_basic_block(fn_val, "entry");
        compiler.builder.position_at_end(entry);
        compiler.function_signatures = Some(fn_val);
        compiler.enter_scope();
        let p = create_entry_block_alloca(&mut compiler, "p").expect("p");
        compiler.build_runtime_value_store(
            p,
            StoreTag::Int(Tag::RawPtr as u64),
            StoreValue::Int(context.i64_type().const_int(0, false)),
            "ptr",
        );
        compiler.add_variable("p".into(), p.into());
        let _ = compiler.get_runtime_fn(&module, "__panic").expect("panic");
        compiler
            .compile_block(&func.body, &module)
            .expect("compile");
        let ir = module.print_to_string().to_string();
        assert!(
            ir.contains("overflow.i64(i64 16,")
                || ir.contains(", i64 16)")
                || ir.contains("i64 16"),
            "expected ABI stride 16, not field-size sum\n{ir}"
        );
    }

    #[test]
    fn take_moves_storage_without_unit_tag() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "deref_take.sprs".into());
        let module = context.create_module("deref_take_test");
        let taken = expr(
            hir::ExprKind::Macro("take".into(), vec![mu_deref_p()]),
            Type::TypeI64,
        );
        let body = vec![stmt(hir::StmtKind::Return(Some(taken)))];
        compile_deref_fixture(
            &mut compiler,
            &context,
            &module,
            body,
            "deref_take",
            41,
            0,
            ptr_mu_i64(),
        );
        let ir = module.print_to_string().to_string();
        assert!(
            !ir.contains("store i32 6"),
            "take must not write Unit\n{ir}"
        );
        assert!(
            !ir.contains("call {{ i32, i64 }} @__clone")
                && !ir.contains("call { i32, i64 } @__clone"),
            "{ir}"
        );
        let result = jit_i64(&module, "deref_take");
        assert_eq!(result.tag, Tag::Integer as i32);
        assert_eq!(result.data, 41);
    }

    #[test]
    fn init_ref_take_through_offset() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "deref_init.sprs".into());
        let module = context.create_module("deref_init_test");
        let dest = expr(
            hir::ExprKind::Deref(Box::new(add_one(ptr_mu_i64()))),
            Type::App("MaybeUninit".into(), vec![Type::TypeI64]),
        );
        let init = expr(
            hir::ExprKind::Macro(
                "init".into(),
                vec![dest, expr(hir::ExprKind::Number(41), Type::TypeI64)],
            ),
            Type::Unit,
        );
        let referred = expr(
            hir::ExprKind::Macro(
                "ref".into(),
                vec![expr(
                    hir::ExprKind::Deref(Box::new(add_one(ptr_mu_i64()))),
                    Type::App("MaybeUninit".into(), vec![Type::TypeI64]),
                )],
            ),
            ptr_i64(),
        );
        let body = vec![
            stmt(hir::StmtKind::Expr(init)),
            stmt(hir::StmtKind::Return(Some(expr(
                hir::ExprKind::Deref(Box::new(referred)),
                Type::TypeI64,
            )))),
        ];
        compile_deref_fixture(
            &mut compiler,
            &context,
            &module,
            body,
            "deref_init",
            0,
            0,
            ptr_mu_i64(),
        );
        let ir = module.print_to_string().to_string();
        assert!(
            !ir.contains("@init destination is already initialized"),
            "{ir}"
        );
        assert!(!ir.contains("init_already"), "{ir}");
        let result = jit_i64(&module, "deref_init");
        assert_eq!(result.tag, Tag::Integer as i32);
        assert_eq!(result.data, 41);
    }

    #[test]
    fn owned_str_init_ref_take_drops_once() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "owned_str.sprs".into());
        let module = context.create_module("owned_str_test");
        let ptr_ty = Type::App(
            "Ptr".into(),
            vec![Type::App("MaybeUninit".into(), vec![Type::Str])],
        );
        let mu = Type::App("MaybeUninit".into(), vec![Type::Str]);
        let init = expr(
            hir::ExprKind::Macro(
                "init".into(),
                vec![
                    expr(
                        hir::ExprKind::Deref(Box::new(expr(
                            hir::ExprKind::Var("p".into()),
                            ptr_ty.clone(),
                        ))),
                        mu.clone(),
                    ),
                    expr(hir::ExprKind::Str("hello".into()), Type::Str),
                ],
            ),
            Type::Unit,
        );
        let referred = expr(
            hir::ExprKind::Macro(
                "ref".into(),
                vec![expr(
                    hir::ExprKind::Deref(Box::new(expr(
                        hir::ExprKind::Var("p".into()),
                        ptr_ty.clone(),
                    ))),
                    mu.clone(),
                )],
            ),
            Type::App("Ptr".into(), vec![Type::Str]),
        );
        let taken = expr(
            hir::ExprKind::Macro(
                "take".into(),
                vec![expr(
                    hir::ExprKind::Deref(Box::new(expr(
                        hir::ExprKind::Var("p".into()),
                        ptr_ty.clone(),
                    ))),
                    mu,
                )],
            ),
            Type::Str,
        );
        let body = vec![
            stmt(hir::StmtKind::Expr(init)),
            stmt(hir::StmtKind::Var {
                name: "q".into(),
                binding_ty: Type::App("Ptr".into(), vec![Type::Str]),
                is_ambi: false,
                is_annotated: true,
                init: referred,
            }),
            stmt(hir::StmtKind::Var {
                name: "s".into(),
                binding_ty: Type::Str,
                is_ambi: false,
                is_annotated: true,
                init: taken,
            }),
            stmt(hir::StmtKind::Return(Some(expr(
                hir::ExprKind::Number(1),
                Type::TypeI64,
            )))),
        ];
        let func = hir::Function {
            name: "owned_str".into(),
            params: Vec::new(),
            body,
            ret_ty: Some(Type::Int),
            is_public: true,
            type_params: Vec::new(),
            when_rules: Vec::new(),
            span: dummy_span(),
        };
        compiler.declare_fn_prototype(&func, &module);
        let fn_val = module.get_function("owned_str").expect("prototype");
        let entry = context.append_basic_block(fn_val, "entry");
        compiler.builder.position_at_end(entry);
        compiler.function_signatures = Some(fn_val);
        compiler.enter_scope();
        let handle_slot = compiler
            .builder
            .build_alloca(context.i64_type(), "str_slot")
            .unwrap();
        let p = create_entry_block_alloca(&mut compiler, "p").expect("p");
        let addr = compiler
            .builder
            .build_ptr_to_int(handle_slot, context.i64_type(), "str_addr")
            .unwrap();
        compiler.build_runtime_value_store(
            p,
            StoreTag::Int(Tag::RawPtr as u64),
            StoreValue::Int(addr),
            "ptr",
        );
        compiler.add_variable("p".into(), p.into());
        for name in [
            "__drop",
            "__clone",
            "__panic",
            "__string_new",
            "__struct_new",
            "__struct_borrow",
            "__struct_track_value",
            "__struct_forget_owned",
        ] {
            let _ = compiler.get_runtime_fn(&module, name).expect(name);
        }
        compiler
            .compile_block(&func.body, &module)
            .expect("compile");
        if compiler
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            compiler.exit_scope(&module).expect("exit");
        } else if !compiler.scopes.is_empty() {
            compiler.scopes.pop();
        }
        assert!(fn_val.verify(true), "invalid owned_str");
        let ir = module.print_to_string().to_string();
        let news = ir.matches("call i64 @__string_from_cstr").count();
        assert_eq!(
            news, 1,
            "init/take must not allocate a new string slab\n{ir}"
        );
        assert!(
            !ir.contains("call { i32, i64 } @__clone"),
            "take/init must not clone\n{ir}"
        );
        Target::initialize_native(&InitializationConfig::default()).expect("native target");
        let engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .expect("jit");
        map_runtime(&engine, &module);
        if let Some(f) = module.get_function("__string_new") {
            engine.add_global_mapping(&f, sprs_runtime::__string_new as *const () as usize);
        }
        if let Some(f) = module.get_function("__string_from_cstr") {
            engine.add_global_mapping(&f, sprs_runtime::__string_from_cstr as *const () as usize);
        }
        type TestFn = unsafe extern "C" fn() -> SprsValue;
        let f = unsafe { engine.get_function::<TestFn>("owned_str") }.expect("lookup");
        let result = unsafe { f.call() };
        assert_eq!(result.tag, Tag::Integer as i32);
        assert_eq!(result.data, 1);
    }

    #[test]
    fn struct_owned_field_inline_layout_and_take() {
        let context = Context::create();
        let builder = context.create_builder();
        let mut compiler = Compiler::new(&context, builder, "user.sprs".into());
        let module = context.create_module("user_test");
        compiler
            .register_struct(
                "User".into(),
                vec![
                    crate::front::ast::StructField {
                        ident: "id".into(),
                        ty: Some(Type::TypeI64),
                        default_value: None,
                        span: dummy_span(),
                    },
                    crate::front::ast::StructField {
                        ident: "name".into(),
                        ty: Some(Type::Str),
                        default_value: None,
                        span: dummy_span(),
                    },
                ],
            )
            .unwrap();
        let user_ty = Type::Struct("User".into());
        let layout = compiler.storage_layout(&user_ty).unwrap();
        assert_eq!(layout.align, 8);
        assert!(layout.size >= 12);
        let ptr_ty = Type::App(
            "Ptr".into(),
            vec![Type::App("MaybeUninit".into(), vec![user_ty.clone()])],
        );
        let mu = Type::App("MaybeUninit".into(), vec![user_ty.clone()]);
        let init_struct = expr(
            hir::ExprKind::StructInit {
                struct_ref: hir::StructRef::Plain("User".into()),
                fields: vec![
                    (0, expr(hir::ExprKind::Number(7), Type::TypeI64)),
                    (1, expr(hir::ExprKind::Str("n".into()), Type::Str)),
                ],
            },
            user_ty.clone(),
        );
        let body = vec![
            stmt(hir::StmtKind::Var {
                name: "u".into(),
                binding_ty: user_ty.clone(),
                is_ambi: false,
                is_annotated: true,
                init: init_struct,
            }),
            stmt(hir::StmtKind::Expr(expr(
                hir::ExprKind::Macro(
                    "init".into(),
                    vec![
                        expr(
                            hir::ExprKind::Deref(Box::new(expr(
                                hir::ExprKind::Var("p".into()),
                                ptr_ty.clone(),
                            ))),
                            mu.clone(),
                        ),
                        expr(hir::ExprKind::Var("u".into()), user_ty.clone()),
                    ],
                ),
                Type::Unit,
            ))),
            stmt(hir::StmtKind::Var {
                name: "v".into(),
                binding_ty: user_ty.clone(),
                is_ambi: false,
                is_annotated: true,
                init: expr(
                    hir::ExprKind::Macro(
                        "take".into(),
                        vec![expr(
                            hir::ExprKind::Deref(Box::new(expr(
                                hir::ExprKind::Var("p".into()),
                                ptr_ty.clone(),
                            ))),
                            mu,
                        )],
                    ),
                    user_ty,
                ),
            }),
            stmt(hir::StmtKind::Return(Some(expr(
                hir::ExprKind::Number(1),
                Type::TypeI64,
            )))),
        ];
        let func = hir::Function {
            name: "user_flow".into(),
            params: Vec::new(),
            body,
            ret_ty: Some(Type::Int),
            is_public: true,
            type_params: Vec::new(),
            when_rules: Vec::new(),
            span: dummy_span(),
        };
        compiler.declare_fn_prototype(&func, &module);
        let fn_val = module.get_function("user_flow").expect("prototype");
        let entry = context.append_basic_block(fn_val, "entry");
        compiler.builder.position_at_end(entry);
        compiler.function_signatures = Some(fn_val);
        compiler.enter_scope();
        let slot = compiler
            .builder
            .build_alloca(layout.llvm_type, "user_slot")
            .unwrap();
        let p = create_entry_block_alloca(&mut compiler, "p").expect("p");
        let addr = compiler
            .builder
            .build_ptr_to_int(slot, context.i64_type(), "user_addr")
            .unwrap();
        compiler.build_runtime_value_store(
            p,
            StoreTag::Int(Tag::RawPtr as u64),
            StoreValue::Int(addr),
            "ptr",
        );
        compiler.add_variable("p".into(), p.into());
        for name in [
            "__drop",
            "__clone",
            "__panic",
            "__string_new",
            "__struct_new",
            "__struct_borrow",
            "__struct_track_value",
            "__struct_forget_owned",
        ] {
            let _ = compiler.get_runtime_fn(&module, name).expect(name);
        }
        compiler
            .compile_block(&func.body, &module)
            .expect("compile");
        if compiler
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            compiler.exit_scope(&module).expect("exit");
        } else if !compiler.scopes.is_empty() {
            compiler.scopes.pop();
        }
        assert!(fn_val.verify(true), "invalid user_flow");
        let ir = module.print_to_string().to_string();
        let news = ir.matches("call i64 @__string_from_cstr").count();
        assert_eq!(
            news, 1,
            "string payload must not be cloned for init/take\n{ir}"
        );
        Target::initialize_native(&InitializationConfig::default()).expect("native target");
        let engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .expect("jit");
        map_runtime(&engine, &module);
        if let Some(f) = module.get_function("__string_new") {
            engine.add_global_mapping(&f, sprs_runtime::__string_new as *const () as usize);
        }
        if let Some(f) = module.get_function("__string_from_cstr") {
            engine.add_global_mapping(&f, sprs_runtime::__string_from_cstr as *const () as usize);
        }
        type TestFn = unsafe extern "C" fn() -> SprsValue;
        let f = unsafe { engine.get_function::<TestFn>("user_flow") }.expect("lookup");
        let result = unsafe { f.call() };
        assert_eq!(result.tag, Tag::Integer as i32);
        assert_eq!(result.data, 1);
    }
}

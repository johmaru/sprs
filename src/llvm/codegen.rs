use crate::front::ast;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::span::Spanned;
use crate::front::type_helper;
use crate::front::type_helper::{Type, types_compatible};
use crate::llvm::builder_helper;
use crate::llvm::builder_helper::Comparison;
use crate::llvm::builder_helper::EqNeq;
use crate::llvm::builder_helper::UpDown;
use crate::llvm::compiler::{Compiler, OS, Tag};
use crate::llvm::value::create_label;
use crate::naming;
use inkwell::AddressSpace;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};

impl<'ctx> Compiler<'ctx> {
    pub fn get_known_type_from_expr(&self, expr: &Spanned<ast::Expr>) -> Result<String, SprsError> {
        match &expr.node {
            ast::Expr::TypeI8 => Ok("i8".to_string()),
            ast::Expr::TypeU8 => Ok("u8".to_string()),
            ast::Expr::TypeI16 => Ok("i16".to_string()),
            ast::Expr::TypeU16 => Ok("u16".to_string()),
            ast::Expr::TypeI32 => Ok("i32".to_string()),
            ast::Expr::TypeU32 => Ok("u32".to_string()),
            ast::Expr::TypeI64 => Ok("i64".to_string()),
            ast::Expr::TypeU64 => Ok("u64".to_string()),

            ast::Expr::TypeF16 => Ok("fp16".to_string()),
            ast::Expr::TypeF32 => Ok("fp32".to_string()),
            ast::Expr::TypeF64 => Ok("fp64".to_string()),

            ast::Expr::Number(_) => Ok("default(i64)".to_string()),
            _ => Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 1,
                },
                location: Location::new(String::new(), expr.span),
                message: format!("Unknown type expression for known type: {:?}", expr),
                help: None,
            }),
        }
    }

    pub fn get_expr_name(&self, expr: &Spanned<ast::Expr>) -> Option<String> {
        match &expr.node {
            ast::Expr::Var(name) => Some(name.clone()),
            _ => None,
        }
    }

    /// Check call arity and annotated parameter types against argument expressions.
    pub fn check_call_arguments(
        &self,
        fn_name: &str,
        args: &[Spanned<ast::Expr>],
    ) -> Result<(), SprsError> {
        let Some(sig) = self.fn_types.get(fn_name) else {
            return Ok(());
        };

        let expected = sig.params.len();
        let actual = args.len();
        if expected != actual {
            let span = args.first().map(|a| a.span).unwrap_or(Span::DUMMY);
            return Err(SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 16,
                },
                location: Location::new(String::new(), span),
                message: format!(
                    "Argument count mismatch: function `{}` expects {} argument(s), found {}",
                    fn_name, expected, actual
                ),
                help: None,
            });
        }

        for (idx, (param_annot, arg)) in sig.params.iter().zip(args.iter()).enumerate() {
            let Some(annot) = param_annot else {
                continue;
            };
            let actual_ty = self.infer_type(arg);
            if !types_compatible(&annot.ty, &actual_ty) {
                return Err(SprsError::Type {
                    code: ErrorCode {
                        category: ErrorCategory::Type,
                        number: 7,
                    },
                    location: Location::new(String::new(), arg.span),
                    message: format!(
                        "Type mismatch: argument {} of `{}` expects {:?}, found {:?}",
                        idx + 1,
                        fn_name,
                        annot.ty,
                        actual_ty
                    ),
                    expected_type: Some(format!("{:?}", annot.ty)),
                    actual_type: Some(format!("{:?}", actual_ty)),
                    help: None,
                });
            }
        }
        Ok(())
    }

    fn infer_type(&self, expr: &Spanned<ast::Expr>) -> Type {
        match &expr.node {
            ast::Expr::Number(_) => Type::Int,
            ast::Expr::Float(_) => Type::Float,
            ast::Expr::Str(_) => Type::Str,
            ast::Expr::Bool(_) => Type::Bool,
            ast::Expr::Unit() => Type::Unit,
            ast::Expr::Var(name) => self.get_variables(name).map(|b| b.ty).unwrap_or(Type::Any),
            ast::Expr::TypeI8 => Type::TypeI8,
            ast::Expr::TypeU8 => Type::TypeU8,
            ast::Expr::TypeI16 => Type::TypeI16,
            ast::Expr::TypeU16 => Type::TypeU16,
            ast::Expr::TypeI32 => Type::TypeI32,
            ast::Expr::TypeU32 => Type::TypeU32,
            ast::Expr::TypeI64 => Type::TypeI64,
            ast::Expr::TypeU64 => Type::TypeU64,
            ast::Expr::TypeF16 => Type::TypeF16,
            ast::Expr::TypeF32 => Type::TypeF32,
            ast::Expr::TypeF64 => Type::TypeF64,
            ast::Expr::Add(lhs, _)
            | ast::Expr::Mul(lhs, _)
            | ast::Expr::Minus(lhs, _)
            | ast::Expr::Div(lhs, _)
            | ast::Expr::Mod(lhs, _) => self.infer_type(lhs),
            ast::Expr::Increment(value) | ast::Expr::Decrement(value) | ast::Expr::Neg(value) => {
                self.infer_type(value)
            }
            ast::Expr::If(_, then, if_else) => {
                let then_ty = self.infer_type(then);
                let else_ty = self.infer_type(if_else);
                if types_compatible(&then_ty, &else_ty) {
                    // Prefer the more specific side when one is a default-width alias.
                    if then_ty != Type::Any {
                        then_ty
                    } else {
                        else_ty
                    }
                } else {
                    Type::Any
                }
            }
            ast::Expr::Call(name, _) => self
                .fn_types
                .get(name)
                .and_then(|sig| sig.ret_ty.clone())
                .unwrap_or(Type::Any),
            ast::Expr::ModuleAccess(_, function_name, _) => self
                .fn_types
                .get(function_name)
                .and_then(|sig| sig.ret_ty.clone())
                .unwrap_or(Type::Any),
            ast::Expr::Macro(ident, args) => match ident.as_str() {
                "cast" => {
                    if args.len() >= 2 {
                        self.infer_type(&args[1])
                    } else {
                        Type::Any
                    }
                }
                "lshift" | "rshift" => {
                    if !args.is_empty() {
                        self.infer_type(&args[0])
                    } else {
                        Type::Any
                    }
                }
                "not" => Type::Bool,
                "error" => Type::Error,
                "label_is" => Type::Bool,
                "label_name" => Type::Str,
                "label_payload" => Type::Any,
                "init" => Type::Any,
                _ => Type::Any,
            },
            ast::Expr::List(_) => Type::List,
            ast::Expr::Range(_, _) => Type::Range,
            // On the continuing path after `?`, the value has the inner type;
            // Error propagates by returning from the function.
            ast::Expr::Try(inner) => self.infer_type(inner),
            ast::Expr::StructInit(name, _) => Type::Struct(name.clone()),
            ast::Expr::Label(_, None) => Type::Label,
            ast::Expr::Label(_, Some(payload)) => {
                let payload_ty = self.infer_type(payload);
                if matches!(payload_ty, Type::Unit) {
                    Type::Label
                } else {
                    Type::App("Label".into(), vec![payload_ty])
                }
            }
            _ => Type::Any,
        }
    }

    pub fn compile_fn(
        &mut self,
        func: &ast::Function,
        module: &Module<'ctx>,
    ) -> Result<FunctionValue<'ctx>, SprsError> {
        let arg_types: Vec<BasicMetadataTypeEnum> = (0..func.params.len())
            .map(|_| self.context.ptr_type(AddressSpace::default()).into())
            .collect();

        let func_name = if func.ident == "main" {
            naming::INTERNAL_MAIN_FN
        } else {
            &func.ident
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

        let fn_sig = self.fn_types.get(func_name).cloned();
        self.current_fn_ret_ty = func.ret_ty.clone();

        for (idx, param) in func.params.iter().enumerate() {
            let arg_val = fn_val.get_nth_param(idx as u32).unwrap();
            // Params are declared as pointers to SprsValue (see declare_fn_prototype).
            let arg_ptr = arg_val.into_pointer_value();

            let alloca = self
                .builder
                .build_alloca(self.runtime_value_type, &param.ident)
                .unwrap();
            let loaded = self
                .builder
                .build_load(self.runtime_value_type, arg_ptr, &param.ident)
                .unwrap();
            self.builder
                .build_store(alloca, loaded)
                .map_err(|e| SprsError::Internal {
                    message: e.to_string(),
                    location: None,
                })?;
            let annot = param.ty.clone().or_else(|| {
                fn_sig
                    .as_ref()
                    .and_then(|s| s.params.get(idx).cloned().flatten())
            });
            let (param_ty, is_ambi, is_annotated) = match annot {
                Some(a) => (a.ty, a.ambi, true),
                None => (Type::Any, false, false),
            };
            self.add_variable(
                param.ident.clone(),
                alloca.into(),
                param_ty,
                false,
                is_ambi,
                is_annotated,
            );
        }

        self.compile_block(&func.blk, module)?;
        let current_block = self.builder.get_insert_block().unwrap();
        if current_block.get_terminator().is_none() {
            // Inter compile_block will execute exit_scope, so need scope of function args end here
            self.emit_drop_for_attachments(module)?;
            self.exit_scope(module)?;
            builder_helper::create_dummy_for_no_return(self);
        } else {
            self.scopes.pop();
        }
        self.attachments.clear();

        self.current_fn_ret_ty = None;
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

    /// Process a `return` statement: type-check the expression, convert it
    /// to the function's return type, emit drops, and build the `ret` instr.
    fn compile_return(
        &mut self,
        expr_opt: &Option<Spanned<ast::Expr>>,
        module: &Module<'ctx>,
    ) -> Result<(), SprsError> {
        let ret_val = if let Some(expr) = expr_opt {
            let mut ptr = self.compile_expr(expr, module)?.into_pointer_value();

            if let ast::Expr::Var(name) = &expr.node {
                let binding = self.get_variables(name).unwrap();
                let val_ptr = binding.value.into_pointer_value();

                if binding.always_clone {
                    ptr = builder_helper::clone_runtime_value(self, val_ptr, module)?;
                } else {
                    // Copy first, then invalidate the binding. Unit-ing before
                    // convert_return_value would return Unit.
                    ptr = builder_helper::var_load_at_init_variable(self, val_ptr, "ret_move")?;
                    builder_helper::var_return_store(self, &val_ptr.into(), name);
                };
            }

            let current_fn = self.function_signatures.unwrap();
            let return_type = current_fn.get_type().get_return_type();
            let expr_type = self.infer_type(expr);
            let expected_ret = self.current_fn_ret_ty.clone();

            self.validate_sprs_return_type(&expected_ret, expr_type.clone(), expr)?;
            self.validate_return_type(return_type, expr_type, expr)?;

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

    /// Check a `return` expression against the Sprs `>> T` annotation.
    ///
    /// `Tag::Error` may always propagate (catchable errors), so `Error` is allowed
    /// alongside the declared success type. `Any` (unknown) is not rejected.
    fn validate_sprs_return_type(
        &self,
        expected: &Option<Type>,
        actual: Type,
        expr: &Spanned<ast::Expr>,
    ) -> Result<(), SprsError> {
        let Some(expected_ty) = expected else {
            return Ok(());
        };
        if actual == Type::Any || actual == Type::Error || types_compatible(expected_ty, &actual) {
            return Ok(());
        }
        Err(SprsError::Type {
            code: ErrorCode {
                category: ErrorCategory::Type,
                number: 5,
            },
            location: Location::new(String::new(), expr.span),
            message: format!(
                "Type mismatch: Function declares >> {:?} but return expression has {:?}",
                expected_ty, actual
            ),
            expected_type: Some(format!("{:?}", expected_ty)),
            actual_type: Some(format!("{:?}", actual)),
            help: None,
        })
    }

    /// Validate that the expression type matches the LLVM function return type.
    ///
    /// After catchable errors, Sprs functions always use `runtime_value_type` as
    /// the LLVM return type, so the int/float/pointer branches below are mainly
    /// for residual / non-Sprs ABI cases. Prefer [`validate_sprs_return_type`]
    /// for `>> T` checking.
    fn validate_return_type(
        &self,
        return_type: Option<BasicTypeEnum<'ctx>>,
        expr_type: Type,
        expr: &Spanned<ast::Expr>,
    ) -> Result<(), SprsError> {
        if let Some(ret_ty) = return_type {
            if ret_ty.is_pointer_type() {
                let llvm_int_ty = type_helper::is_int_type_in_llvm();
                if llvm_int_ty.contains(&expr_type) {
                    return Err(SprsError::Type {
                        code: ErrorCode {
                            category: ErrorCategory::Type,
                            number: 1,
                        },
                        location: Location::new(String::new(), expr.span),
                        message: format!(
                            "Type mismatch: Function expects pointer type (e.g. str) but got {:?} from expression {:?}",
                            expr_type, expr
                        ),
                        expected_type: Some("pointer".to_string()),
                        actual_type: Some(format!("{:?}", expr_type)),
                        help: None,
                    });
                }
            } else if ret_ty.is_int_type() {
                let width = ret_ty.into_int_type().get_bit_width();
                if width == 1 {
                    if expr_type != Type::Bool {
                        return Err(SprsError::Type {
                            code: ErrorCode {
                                category: ErrorCategory::Type,
                                number: 2,
                            },
                            location: Location::new(String::new(), expr.span),
                            message: format!(
                                "Type mismatch: Function expects Bool but got {:?} from expression {:?}",
                                expr_type, expr
                            ),
                            expected_type: Some("Bool".to_string()),
                            actual_type: Some(format!("{:?}", expr_type)),
                            help: None,
                        });
                    }
                } else {
                    let llvm_not_int = type_helper::not_int_type_in_llvm();
                    if llvm_not_int.contains(&expr_type) {
                        return Err(SprsError::Type {
                            code: ErrorCode {
                                category: ErrorCategory::Type,
                                number: 3,
                            },
                            location: Location::new(String::new(), expr.span),
                            message: format!(
                                "Type mismatch: Function expects Int type but got {:?} from expression {:?}",
                                expr_type, expr
                            ),
                            expected_type: Some("Int".to_string()),
                            actual_type: Some(format!("{:?}", expr_type)),
                            help: None,
                        });
                    }
                }
            } else if ret_ty.is_float_type() {
                let llvm_float_ty = type_helper::is_float_type_in_llvm();
                if !llvm_float_ty.contains(&expr_type) {
                    return Err(SprsError::Type {
                        code: ErrorCode {
                            category: ErrorCategory::Type,
                            number: 4,
                        },
                        location: Location::new(String::new(), expr.span),
                        message: format!(
                            "Type mismatch: Function expects Float type but got {:?} from expression {:?}",
                            expr_type, expr
                        ),
                        expected_type: Some("Float".to_string()),
                        actual_type: Some(format!("{:?}", expr_type)),
                        help: None,
                    });
                }
            }
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
        stmts: &Vec<Spanned<ast::Stmt>>,
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

            match &stmt.node {
                ast::Stmt::Var(var) => {
                    let unit_expr = Spanned::new(ast::Expr::Unit(), Span::DUMMY);
                    let init_expr = var.expr.as_ref().unwrap_or(&unit_expr);
                    let compiled_init_val =
                        self.compile_expr(init_expr, module)?.into_pointer_value();

                    let var_type = self.infer_type(init_expr);

                    if var.always_clone {
                        let expensive_cp = matches!(var_type, Type::Struct(_) | Type::Enum(_))
                            || matches!(
                                &init_expr.node,
                                ast::Expr::List(_)
                                    | ast::Expr::Range(_, _)
                                    | ast::Expr::StructInit(_, _)
                            );
                        if expensive_cp {
                            eprintln!(
                                "warning: `cp` on non-str heap value `{}` deep-copies on every use (Phase 1 recommends `cp` mainly for `str`)",
                                var.ident
                            );
                        }
                    }

                    let init_val = if let ast::Expr::Var(src_val_name) = &init_expr.node {
                        let src = self
                            .get_variables(src_val_name)
                            .ok_or_else(|| format!("Undefined variable: {}", src_val_name))?;

                        if src.always_clone {
                            builder_helper::clone_runtime_value(
                                self,
                                src.value.into_pointer_value(),
                                module,
                            )?
                        } else {
                            let copied_val = builder_helper::var_load_at_init_variable(
                                self,
                                compiled_init_val,
                                &var.ident,
                            )?;
                            builder_helper::move_variable(self, &src.value, &var.ident);
                            copied_val
                        }
                    } else {
                        builder_helper::var_load_at_init_variable(
                            self,
                            compiled_init_val,
                            &var.ident,
                        )?
                    };
                    self.add_variable(
                        var.ident.clone(),
                        init_val.into(),
                        var_type,
                        var.always_clone,
                        false,
                        false,
                    );
                }
                ast::Stmt::Return(expr_opt) => {
                    self.compile_return(expr_opt, module)?;
                }
                ast::Stmt::If {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    builder_helper::create_if_condition(self, cond, then_blk, else_blk, module)
                        .map_err(|e| e.to_string())?;
                }
                ast::Stmt::While { cond, body } => {
                    builder_helper::create_while_condition(self, cond, body, module)
                        .map_err(|e| e.to_string())?;
                }
                ast::Stmt::Expr(expr) => {
                    self.compile_expr(expr, module)?;
                }
                ast::Stmt::EnumItem(enm) => {
                    self.register_enum(enm, &module, false);
                }
                ast::Stmt::Assign(assign_stmt) => {
                    // Self-assign is a no-op: drop-then-load on the same binding
                    // would destroy the value.
                    if let ast::Expr::Var(src_val_name) = &assign_stmt.expr.node {
                        if src_val_name == &assign_stmt.name {
                            continue;
                        }
                    }

                    let mut val_ptr = self
                        .compile_expr(&assign_stmt.expr, module)?
                        .into_pointer_value();

                    // For non-cp sources, move must happen AFTER we copy bits into the
                    // target. Moving first would Unit the source while val_ptr still
                    // aliases it, so the assignment stores Unit.
                    let mut source_to_move: Option<(BasicValueEnum<'ctx>, String)> = None;
                    if let ast::Expr::Var(src_val_name) = &assign_stmt.expr.node {
                        let src = self
                            .get_variables(src_val_name)
                            .ok_or_else(|| format!("Undefined variable: {}", src_val_name))?;
                        if src.always_clone {
                            val_ptr = builder_helper::clone_runtime_value(
                                self,
                                src.value.into_pointer_value(),
                                module,
                            )?;
                        } else {
                            source_to_move = Some((src.value, src_val_name.clone()));
                        }
                    }

                    let target = self
                        .get_variables(&assign_stmt.name)
                        .ok_or_else(|| format!("Undefined variable: {}", &assign_stmt.name))?;

                    let rhs_ty = self.infer_type(&assign_stmt.expr);
                    if target.is_annotated && !target.is_ambi {
                        if !types_compatible(&target.ty, &rhs_ty) {
                            return Err(SprsError::Type {
                                code: ErrorCode {
                                    category: ErrorCategory::Type,
                                    number: 6,
                                },
                                location: Location::new(String::new(), assign_stmt.span),
                                message: format!(
                                    "Type mismatch: cannot assign {:?} to fixed binding `{}` of type {:?}",
                                    rhs_ty, assign_stmt.name, target.ty
                                ),
                                expected_type: Some(format!("{:?}", target.ty)),
                                actual_type: Some(format!("{:?}", rhs_ty)),
                                help: Some(
                                    "use `>> ambi T` if this parameter should allow dynamic reassignment"
                                        .to_string(),
                                ),
                            });
                        }
                    }

                    let target_ptr = target.value.into_pointer_value();

                    let drop_fn = self.get_runtime_fn(module, "__drop")?;
                    builder_helper::drop_var(self, target_ptr, drop_fn, &assign_stmt.name);

                    let new_val = self
                        .builder
                        .build_load(self.runtime_value_type, val_ptr, "assign_load")
                        .unwrap();
                    self.builder
                        .build_store(target_ptr, new_val)
                        .map_err(|e| e.to_string())?;

                    if let Some((val, src_name)) = source_to_move {
                        builder_helper::move_variable(self, &val, &src_name);
                    }

                    // Update static type: ambi / unannotated bindings track the RHS.
                    if !target.is_annotated || target.is_ambi {
                        self.set_variable_type(&assign_stmt.name, rhs_ty);
                    }
                }
            }
        }

        self.exit_scope(module)?;

        Ok(())
    }

    pub(crate) fn compile_expr(
        &mut self,
        expr: &Spanned<ast::Expr>,
        module: &Module<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, SprsError> {
        match &expr.node {
            ast::Expr::Number(n) => Ok(builder_helper::create_integer(self, n)?),
            ast::Expr::Float(fp) => Ok(builder_helper::create_float(self, *fp)?),
            ast::Expr::TypeI8 => builder_helper::create_int8(self),
            ast::Expr::TypeU8 => builder_helper::create_uint8(self),
            ast::Expr::TypeI16 => builder_helper::create_int16(self),
            ast::Expr::TypeU16 => builder_helper::create_uint16(self),
            ast::Expr::TypeI32 => builder_helper::create_int32(self),
            ast::Expr::TypeU32 => builder_helper::create_uint32(self),
            ast::Expr::TypeI64 => builder_helper::create_int64(self),
            ast::Expr::TypeU64 => builder_helper::create_uint64(self),
            ast::Expr::TypeF16 => builder_helper::create_float16(self),
            ast::Expr::TypeF32 => builder_helper::create_float32(self),
            ast::Expr::TypeF64 => builder_helper::create_float64(self),
            ast::Expr::Str(str) => Ok(builder_helper::create_string(self, str, module)?),
            ast::Expr::Bool(boolean) => Ok(builder_helper::create_bool(self, boolean)?),
            ast::Expr::Var(ident) => {
                if let Some(binding) = self.get_variables(ident) {
                    Ok(binding.value)
                } else {
                    Err(SprsError::Semantic {
                        code: ErrorCode { category: ErrorCategory::Semantic, number: 2 },
                        location: Location::new(String::new(), expr.span),
                        message: format!("Undefined variable: {}", ident),
                        help: None,
                    })
                }
            }
            ast::Expr::Call(ident, args) => Ok(builder_helper::create_call_expr(self, ident, args, module)?),
            ast::Expr::Macro(ident, args) => {
                match ident.as_str() {
                    "println" => Ok(builder_helper::call_builtin_macro_println(self, args, module)?),
                    "list_push" => Ok(builder_helper::call_builtin_macro_list_push(self, args, module)?),
                    "clone" => Ok(builder_helper::call_builtin_macro_clone(self, args, module)?),
                    "move" => Ok(builder_helper::call_builtin_macro_move(self, args, module)?),
                    "cast" => Ok(builder_helper::call_builtin_macro_cast(self, args, module)?),
                    "lshift" => Ok(builder_helper::call_builtin_macro_lshift(self, args, module)?),
                    "rshift" => Ok(builder_helper::call_builtin_macro_rshift(self, args, module)?),
                    "not" => Ok(builder_helper::call_builtin_macro_not(self, args, module)?),
                    "is_error" => Ok(builder_helper::call_builtin_macro_is_error(self, args, module)?),
                    "error_code" => Ok(builder_helper::call_builtin_macro_error_code(self, args, module)?),
                    "error_message" => Ok(builder_helper::call_builtin_macro_error_message(self, args, module)?),
                    "attach" => Ok(builder_helper::call_builtin_macro_attach(self, args, module)?),
                    "label_is" => Ok(builder_helper::call_builtin_macro_label_is(self, args, module)?),
                    "label_payload" => Ok(builder_helper::call_builtin_macro_label_payload(self, args, module)?),
                    "label_name" => Ok(builder_helper::call_builtin_macro_label_name(self, args, module)?),
                    "error" => Ok(builder_helper::call_builtin_macro_error(self, args, module)?),
                    "init" => Err(SprsError::Semantic {
                        code: ErrorCode { category: ErrorCategory::Semantic, number: 5 },
                        location: Location::new(String::new(), expr.span),
                        message: "struct initialization requires @init(TypeName { field: value, ... }) syntax".to_string(),
                        help: None,
                    }),
                    _ => Err(SprsError::Semantic {
                        code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
                        location: Location::new(String::new(), expr.span),
                        message: format!("Unknown macro: {}", ident),
                        help: None,
                    }),
                }
            }
            ast::Expr::FieldAccess(lhs, rhs) => {
                if let ast::Expr::Var(name) = &lhs.node {
                    if self.enum_names.contains(name) {
                        let full_name = format!("{}.{}", name, rhs);
                        if let Some(binding) = self.get_variables(&full_name) {
                            return Ok(binding.value);
                        } else {
                            return Err(SprsError::Semantic {
                                code: ErrorCode { category: ErrorCategory::Semantic, number: 4 },
                                location: Location::new(String::new(), expr.span),
                                message: format!("Undefined enum variant: {}", full_name),
                                help: None,
                            });
                        }
                    }
                }

                let lhs_type = self.infer_type(lhs);

                let struct_name = match lhs_type {
                    Type::Struct(name) => name,
                    _ => {
                        return Err(SprsError::Semantic {
                            code: ErrorCode { category: ErrorCategory::Semantic, number: 2 },
                            location: Location::new(String::new(), lhs.span),
                            message: format!(
                                "Undefined variable: {}",
                                self.get_expr_name(lhs).unwrap_or_default()
                            ),
                            help: None,
                        });
                    }
                };

                let index = self.get_field_index(&struct_name, rhs)?;

                Ok(builder_helper::create_field_access(self, lhs, index, &struct_name, module)?)
            }
            ast::Expr::Add(lhs, rhs) => Ok(builder_helper::create_add_expr(self, lhs, rhs, module)?),
            ast::Expr::Mul(lhs, rhs) => Ok(builder_helper::create_mul_expr(self, lhs, rhs, module)?),
            ast::Expr::Minus(lhs, rhs) => Ok(builder_helper::create_minus_expr(self, lhs, rhs, module)?),
            ast::Expr::Div(lhs, rhs) => Ok(builder_helper::create_div_expr(self, lhs, rhs, module)?),
            ast::Expr::Mod(lhs, rhs) => Ok(builder_helper::create_mod_expr(self, lhs, rhs, module)?),
            ast::Expr::Increment(expr) => {
                Ok(builder_helper::create_increment_or_decrement(self, expr, UpDown::Up, module)?)
            }
            ast::Expr::Decrement(expr) => {
                Ok(builder_helper::create_increment_or_decrement(self, expr, UpDown::Down, module)?)
            }
            ast::Expr::Neg(expr) => {
                let zero = Spanned::new(ast::Expr::Number(0), Span::DUMMY);
                Ok(builder_helper::create_minus_expr(
                    self,
                    &zero,
                    expr,
                    module,
                )?)
            }
            ast::Expr::Eq(lhs, rhs) => {
                Ok(builder_helper::create_eq_or_neq(
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
                )?)
            }
            ast::Expr::Neq(lhs, rhs) => {
                Ok(builder_helper::create_eq_or_neq(
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
                )?)
            }
            ast::Expr::Gt(lhs, rhs) => {
                Ok(builder_helper::create_comparison(
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
                )?)
            }
            ast::Expr::Lt(lhs, rhs) => {
                Ok(builder_helper::create_comparison(
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
                )?)
            }
            ast::Expr::Ge(lhs, rhs) => {
                Ok(builder_helper::create_comparison(
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
                )?)
            }
            ast::Expr::Le(lhs, rhs) => {
                Ok(builder_helper::create_comparison(
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
                )?)
            }
            ast::Expr::If(cond, then_expr, else_expr) => {
                Ok(builder_helper::create_if_expr(self, cond, then_expr, else_expr, module)?)
            }
            ast::Expr::List(elements) => Ok(builder_helper::create_list(self, elements, module)?),
            ast::Expr::Index(collection_expr, index_expr) => {
                Ok(builder_helper::create_index(self, collection_expr, index_expr, module)?)
            }
            ast::Expr::Range(start_expr, end_expr) => Ok(builder_helper::create_range(self, start_expr, end_expr, module)?),
            ast::Expr::ModuleAccess(module_name, function_name, args) => {
                Ok(builder_helper::create_module_access(
                    self,
                    module_name,
                    function_name,
                    args,
                    module,
                )?)
            }
            ast::Expr::Unit() => Ok(builder_helper::create_unit(self)?),
            ast::Expr::Label(name, payload) => {
                if payload.is_none() {
                    if let crate::front::label_name::LabelName::Static(static_name) = name {
                        if let Some(attached) = self.attachments.get(static_name).copied() {
                            return Ok(builder_helper::clone_runtime_value(self, attached, module)?.into());
                        }
                    }
                }
                Ok(create_label(self, name, payload.as_deref(), module)?)
            }
            ast::Expr::StructInit(struct_name, fields) => Ok(builder_helper::create_struct_init(self, struct_name, fields, module)?),
            ast::Expr::Try(inner_expr) => {
                let inner_ptr = self.compile_expr(inner_expr, module)?.into_pointer_value();

                // Load the tag of the inner result.
                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, inner_ptr, 0, "try_tag_ptr")
                    .unwrap();
                let tag_val = self
                    .builder
                    .build_load(self.context.i32_type(), tag_ptr, "try_tag")
                    .unwrap()
                    .into_int_value();

                let error_tag_const = self.context.i32_type().const_int(Tag::Error as u64, false);
                let is_error = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, tag_val, error_tag_const, "try_is_error")
                    .unwrap();

                let current_fn = self.function_signatures.unwrap();
                let propagate_bb = self.context.append_basic_block(current_fn, "try_propagate");
                let continue_bb = self.context.append_basic_block(current_fn, "try_continue");

                let _ = self
                    .builder
                    .build_conditional_branch(is_error, propagate_bb, continue_bb);

                // Propagate: emit drops and return the error value.
                self.builder.position_at_end(propagate_bb);
                self.emit_drop_for_return(module)?;
                let return_type = current_fn.get_type().get_return_type();
                let ret_val = self.convert_return_value(return_type, inner_ptr)?;
                if let Some(val) = ret_val {
                    self.builder.build_return(Some(&val)).unwrap();
                } else {
                    self.builder.build_return(None).unwrap();
                }

                // Continue: the inner value is not an error, use it.
                self.builder.position_at_end(continue_bb);
                Ok(inner_ptr.into())
            }
        }
    }
}

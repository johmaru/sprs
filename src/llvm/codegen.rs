use std::unreachable;

use crate::front::ast;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::label_name::LabelName;
use crate::front::span::Span;
use crate::front::span::Spanned;
use crate::front::type_helper;
use crate::front::type_helper::{
    Type, is_error_label_type, reject_payloadless_label_type, types_compatible,
};
use crate::llvm::builder_helper;
use crate::llvm::builder_helper::BuilderExt;
use crate::llvm::builder_helper::Comparison;
use crate::llvm::builder_helper::ContextExt;
use crate::llvm::builder_helper::EqNeq;
use crate::llvm::builder_helper::UpDown;
use crate::llvm::compiler::{Compiler, Tag};
use crate::llvm::function_build::{CallContractError, resolve_call_contract};
use crate::llvm::value::{build_label_is_error, create_atom, create_label};
use crate::naming;
use inkwell::AddressSpace;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue, ValueKind};

fn is_int_family(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Int
            | Type::TypeI8
            | Type::TypeU8
            | Type::TypeI16
            | Type::TypeU16
            | Type::TypeI32
            | Type::TypeU32
            | Type::TypeI64
            | Type::TypeU64
    )
}

fn is_float_family(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Float | Type::TypeF16 | Type::TypeF32 | Type::TypeF64
    )
}

fn infer_binary_arith_type(lhs: &Type, rhs: &Type) -> Type {
    if is_int_family(lhs) && is_int_family(rhs) {
        if lhs == rhs { lhs.clone() } else { Type::Int }
    } else if is_float_family(lhs) && is_float_family(rhs) {
        if lhs == rhs { lhs.clone() } else { Type::Float }
    } else {
        lhs.clone()
    }
}

impl<'ctx> Compiler<'ctx> {
    pub fn get_expr_name(&self, expr: &Spanned<ast::Expr>) -> Option<String> {
        match &expr.node {
            ast::Expr::Var(name) => Some(name.clone()),
            _ => None,
        }
    }

    /// Check call arity and parameter types against argument expressions.
    ///
    /// Plain functions reuse the same call-contract resolver as FunctionBuild
    /// (empty type parameters / when rules), so arity and annotated parameter
    /// types share one code path.
    pub fn check_call_arguments(
        &self,
        fn_name: &str,
        args: &[Spanned<ast::Expr>],
    ) -> Result<(), SprsError> {
        let Some(sig) = self.fn_types.get(fn_name) else {
            return Ok(());
        };

        let actuals: Vec<Type> = args.iter().map(|arg| self.infer_type(arg)).collect();
        let contract = crate::llvm::function_build::ResolvedFunctionSignature {
            params: sig
                .params
                .iter()
                .map(|ty| ast::FunctionParam {
                    ident: String::new(),
                    ty: ty.clone(),
                    span: Span::DUMMY,
                })
                .collect(),
            ret_ty: sig.ret_ty.clone(),
            is_public: false,
            type_params: sig.type_params.clone(),
            when_rules: sig.when_rules.clone(),
        };
        match resolve_call_contract(&contract, &actuals) {
            Ok(_) => Ok(()),
            Err(CallContractError::Arity { expected, actual }) => {
                let span = args
                    .first()
                    .map(|argument| argument.span)
                    .unwrap_or(Span::DUMMY);
                Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 16,
                    },
                    location: self.location(span),
                    message: format!(
                        "Argument count mismatch: function `{}` expects {} argument(s), found {}",
                        fn_name, expected, actual
                    ),
                    help: None,
                })
            }
            Err(err) => {
                let span = args
                    .first()
                    .map(|argument| argument.span)
                    .unwrap_or(Span::DUMMY);
                let (message, expected_type, actual_type) = match &err {
                    CallContractError::TypeConflict { message } => (
                        format!("Type mismatch in call to `{}`: {}", fn_name, message),
                        None,
                        None,
                    ),
                    CallContractError::UnresolvedTypeParam { name } => (
                        format!(
                            "Type mismatch in call to `{}`: type parameter `{}` was not resolved to a concrete type",
                            fn_name, name
                        ),
                        None,
                        None,
                    ),
                    CallContractError::NotConcrete { message } => (
                        format!("Type mismatch in call to `{}`: {}", fn_name, message),
                        None,
                        None,
                    ),
                    CallContractError::MultipleMatches => (
                        format!(
                            "Type mismatch in call to `{}`: multiple `when` rules matched",
                            fn_name
                        ),
                        None,
                        None,
                    ),
                    CallContractError::Arity { .. } => unreachable!("handled above"),
                };
                Err(SprsError::Type {
                    code: ErrorCode {
                        category: ErrorCategory::Type,
                        number: 7,
                    },
                    location: self.location(span),
                    message,
                    expected_type,
                    actual_type,
                    help: None,
                })
            }
        }
    }

    /// Infer the return type of a call through the FunctionBuild call-contract
    /// resolver. Plain functions (no type params / when rules) fall back to
    /// the declared `ret_ty`; resolution failures yield `Any`.
    fn infer_call_return_type(
        &self,
        name: &str,
        args: &[Spanned<ast::Expr>],
    ) -> Type {
        let Some(sig) = self.fn_types.get(name) else {
            return Type::Any;
        };
        if sig.type_params.is_empty() && sig.when_rules.is_empty() {
            return sig.ret_ty.clone().unwrap_or(Type::Any);
        }
        let actuals: Vec<Type> = args.iter().map(|arg| self.infer_type(arg)).collect();
        let contract = crate::llvm::function_build::ResolvedFunctionSignature {
            params: sig
                .params
                .iter()
                .map(|ty| ast::FunctionParam {
                    ident: String::new(),
                    ty: ty.clone(),
                    span: Span::DUMMY,
                })
                .collect(),
            ret_ty: sig.ret_ty.clone(),
            is_public: false,
            type_params: sig.type_params.clone(),
            when_rules: sig.when_rules.clone(),
        };
        resolve_call_contract(&contract, &actuals)
            .ok()
            .flatten()
            .unwrap_or(Type::Any)
    }

    pub(crate) fn infer_type(&self, expr: &Spanned<ast::Expr>) -> Type {
        match &expr.node {
            ast::Expr::Number(_) => Type::Int,
            ast::Expr::Float(_) => Type::Float,
            ast::Expr::Str(_) => Type::Str,
            ast::Expr::Bool(_) => Type::Bool,
            ast::Expr::Unit() => Type::Unit,
            ast::Expr::Var(name) => {
                if let Some(binding) = self.get_variables(name) {
                    binding.ty
                } else if self.is_visible_atom_def(name) {
                    Type::App("Atom".into(), vec![Type::Atom(name.clone())])
                } else {
                    Type::Any
                }
            }
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
            ast::Expr::Eq(_, _)
            | ast::Expr::Neq(_, _)
            | ast::Expr::Lt(_, _)
            | ast::Expr::Gt(_, _)
            | ast::Expr::Le(_, _)
            | ast::Expr::Ge(_, _) => Type::Bool,
            ast::Expr::Add(lhs, rhs)
            | ast::Expr::Mul(lhs, rhs)
            | ast::Expr::Minus(lhs, rhs)
            | ast::Expr::Div(lhs, rhs)
            | ast::Expr::Mod(lhs, rhs) => {
                infer_binary_arith_type(&self.infer_type(lhs), &self.infer_type(rhs))
            }
            ast::Expr::Assign(_, rhs) => self.infer_type(rhs),
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
            ast::Expr::Match { scrutinee: _, arms } => {
                // Fold the arm value types like Expr::If; incompatible arms
                // fall back to Type::Any.
                let mut result: Option<Type> = None;
                for arm in arms {
                    let arm_ty = self.infer_type(&arm.value);
                    result = Some(match result {
                        None => arm_ty,
                        Some(t) if types_compatible(&t, &arm_ty) => {
                            if t != Type::Any {
                                t
                            } else {
                                arm_ty
                            }
                        }
                        Some(_) => Type::Any,
                    });
                    if result == Some(Type::Any) {
                        break;
                    }
                }
                result.unwrap_or(Type::Any)
            }
            ast::Expr::Call(name, args) => self.infer_call_return_type(name, args),
            ast::Expr::ModuleAccess(_, function_name, args) => {
                self.infer_call_return_type(function_name, args)
            }
            ast::Expr::Macro(ident, args) => match ident.as_str() {
                "cast" => {
                    if args.len() >= 2 {
                        self.infer_type(&args[1])
                    } else {
                        Type::Any
                    }
                }
                "fcast" => Type::Str,
                "lshift" | "rshift" => {
                    if !args.is_empty() {
                        self.infer_type(&args[0])
                    } else {
                        Type::Any
                    }
                }
                "not" => Type::Bool,
                "raw" => Type::RawPtr,
                "free" => Type::Unit,
                "error" => {
                    if args.is_empty() {
                        Type::App("Label".into(), vec![Type::Atom("error".into())])
                    } else {
                        Type::App(
                            "Label".into(),
                            vec![Type::Atom("error".into()), self.infer_type(&args[0])],
                        )
                    }
                }
                "label_is" => Type::Bool,
                "label_name" => Type::Str,
                "label_payload" => Type::Any,
                "init" => Type::Any,
                "clone" => {
                    if !args.is_empty() {
                        self.infer_type(&args[0])
                    } else {
                        Type::Any
                    }
                }
                _ => Type::Any,
            },
            ast::Expr::List(_) => Type::List,
            ast::Expr::Range(_, _) => Type::Range,
            // On the continuing path after `?`, the value has the inner type;
            // Error propagates by returning from the function.
            ast::Expr::Try(inner) => self.infer_type(inner),
            ast::Expr::StructInit(name, _) => Type::Struct(name.clone()),
            ast::Expr::HeapAlloc(_) => Type::Buffer,
            ast::Expr::Destroy(_) => Type::Unit,
            ast::Expr::Exist(_) => Type::Bool,
            ast::Expr::Atom(name) => match name {
                LabelName::Static(static_name) => {
                    match self.resolve_closed_label_member(static_name, expr.span) {
                        Ok(Some(set)) => Type::ClosedLabelSet(set),
                        _ => Type::App("Atom".into(), vec![Type::Atom(static_name.clone())]),
                    }
                }
                LabelName::Dynamic(_) => Type::AtomVal,
            },
            ast::Expr::Label(name, payload) => {
                let payload_ty = self.infer_type(payload);
                match name {
                    LabelName::Static(static_name) => {
                        if matches!(payload_ty, Type::Unit) {
                            Type::App("Label".into(), vec![Type::Atom(static_name.clone())])
                        } else {
                            Type::App(
                                "Label".into(),
                                vec![Type::Atom(static_name.clone()), payload_ty],
                            )
                        }
                    }
                    LabelName::Dynamic(_) => {
                        if matches!(payload_ty, Type::Unit) {
                            Type::Label
                        } else {
                            Type::App("Label".into(), vec![payload_ty])
                        }
                    }
                }
            }
            ast::Expr::AttachSlot(_) => Type::Any,
            ast::Expr::FieldAccess(lhs, rhs) => {
                if let Type::Struct(struct_name) = self.infer_type(lhs) {
                    if let Some(def) = self.struct_defs.get(&struct_name) {
                        if let Some(field) = def.fields.iter().find(|field| field.ident == *rhs) {
                            return field.ty.clone().unwrap_or(Type::Any);
                        }
                    }
                }
                Type::Any
            }
            ast::Expr::Index(collection, _) => match self.infer_type(collection) {
                Type::App(name, args) if name == "List" => match args.as_slice() {
                    [element] => element.clone(),
                    _ => Type::Any,
                },
                _ => Type::Any,
            },
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
        if let Some(ret_ty) = &func.ret_ty {
            reject_payloadless_label_type(ret_ty).map_err(|msg| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 11,
                },
                location: self.location(func.span),
                message: msg,
                help: None,
            })?;
        }
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
                .map_err(|compile_error| SprsError::Internal {
                    message: compile_error.to_string(),
                    location: None,
                })?;
            let annot = param.ty.clone().or_else(|| {
                fn_sig
                    .as_ref()
                    .and_then(|signature| signature.params.get(idx).cloned().flatten())
            });
            if let Some(argument) = &annot {
                reject_payloadless_label_type(&argument.ty).map_err(|msg| SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 11,
                    },
                    location: self.location(param.span),
                    message: msg,
                    help: None,
                })?;
            }
            let (param_ty, is_ambi, is_annotated) = match annot {
                Some(argument) => (argument.ty, argument.ambi, true),
                None => (Type::Any, false, false),
            };
            self.add_variable(
                param.ident.clone(),
                alloca.into(),
                param_ty,
                is_ambi,
                is_annotated,
            );
        }

        self.compile_block(&func.blk, module)?;
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

    pub(crate) fn compile_owned_expr(
        &mut self,
        expr: &Spanned<ast::Expr>,
        module: &Module<'ctx>,
        temp_name: &str,
    ) -> Result<PointerValue<'ctx>, SprsError> {
        let compiled = self.compile_expr(expr, module)?.into_pointer_value();
        let owned_name = match &expr.node {
            ast::Expr::Var(name) | ast::Expr::Assign(name, _) => Some(name.clone()),
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

    /// Process a `return` statement: type-check the expression, convert it
    /// to the function's return type, emit drops, and build the `ret` instr.
    fn compile_return(
        &mut self,
        expr_opt: &Option<Spanned<ast::Expr>>,
        module: &Module<'ctx>,
    ) -> Result<(), SprsError> {
        let ret_val = if let Some(expr) = expr_opt {
            let ptr = self.compile_owned_expr(expr, module, "ret_owned")?;

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
    /// An `:error` label (including the `err` sugar) may always propagate
    /// (catchable errors), so it is allowed alongside the declared success
    /// type. `Any` (unknown) is not rejected.
    fn validate_sprs_return_type(
        &self,
        expected: &Option<Type>,
        actual: Type,
        expr: &Spanned<ast::Expr>,
    ) -> Result<(), SprsError> {
        let Some(expected_ty) = expected else {
            return Ok(());
        };
        if actual == Type::Any
            || is_error_label_type(&actual)
            || types_compatible(expected_ty, &actual)
        {
            return Ok(());
        }
        Err(SprsError::Type {
            code: ErrorCode {
                category: ErrorCategory::Type,
                number: 5,
            },
            location: self.location(expr.span),
            message: format!(
                "Type mismatch: Function declares >> {} but return expression has {}",
                expected_ty, actual
            ),
            expected_type: Some(format!("{}", expected_ty)),
            actual_type: Some(format!("{}", actual)),
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
                        location: self.location(expr.span),
                        message: format!(
                            "Type mismatch: Function expects pointer type (e.g. str) but got {} from expression {:?}",
                            expr_type, expr
                        ),
                        expected_type: Some("pointer".to_string()),
                        actual_type: Some(format!("{}", expr_type)),
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
                            location: self.location(expr.span),
                            message: format!(
                                "Type mismatch: Function expects Bool but got {} from expression {:?}",
                                expr_type, expr
                            ),
                            expected_type: Some("Bool".to_string()),
                            actual_type: Some(format!("{}", expr_type)),
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
                            location: self.location(expr.span),
                            message: format!(
                                "Type mismatch: Function expects Int type but got {} from expression {:?}",
                                expr_type, expr
                            ),
                            expected_type: Some("Int".to_string()),
                            actual_type: Some(format!("{}", expr_type)),
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
                        location: self.location(expr.span),
                        message: format!(
                            "Type mismatch: Function expects Float type but got {} from expression {:?}",
                            expr_type, expr
                        ),
                        expected_type: Some("Float".to_string()),
                        actual_type: Some(format!("{}", expr_type)),
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

                    let init_val = if let ast::Expr::Var(src_val_name) = &init_expr.node {
                        if let Some(src) = self.get_variables(src_val_name) {
                            let copied_val = builder_helper::var_load_at_init_variable(
                                self,
                                compiled_init_val,
                                &var.ident,
                            )?;
                            builder_helper::move_variable(self, &src.value, &var.ident);
                            copied_val
                        } else {
                            builder_helper::var_load_at_init_variable(
                                self,
                                compiled_init_val,
                                &var.ident,
                            )?
                        }
                    } else {
                        builder_helper::var_load_at_init_variable(
                            self,
                            compiled_init_val,
                            &var.ident,
                        )?
                    };
                    self.add_variable(var.ident.clone(), init_val.into(), var_type, false, false);
                }
                ast::Stmt::Return(expr_opt) => {
                    self.compile_return(expr_opt, module)?;
                }
                ast::Stmt::If {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    builder_helper::create_if_condition(self, cond, then_blk, else_blk, module)?;
                }
                ast::Stmt::While { cond, body } => {
                    builder_helper::create_while_condition(self, cond, body, module)?;
                }
                ast::Stmt::Unsafe { body, .. } => {
                    // Always restore depth, including when compile_block returns Err.
                    self.unsafe_depth += 1;
                    let result = self.compile_block(body, module);
                    self.unsafe_depth -= 1;
                    result?;
                }
                ast::Stmt::Defer { expr, .. } => {
                    // Queue only; exit_scope / emit_drop_for_return execute LIFO later.
                    if let Some(scope) = self.scopes.last_mut() {
                        scope.deferred.push(expr.clone());
                    }
                }
                ast::Stmt::Match {
                    scrutinee,
                    bind,
                    arms,
                    ..
                } => {
                    builder_helper::create_match_stmt(self, scrutinee, bind, arms, module)?;
                }
                ast::Stmt::Expr(expr) => {
                    self.compile_expr(expr, module)?;
                }
                ast::Stmt::Assign(assign_stmt) => {
                    self.emit_named_assign(
                        &assign_stmt.name,
                        &assign_stmt.expr,
                        module,
                        assign_stmt.span,
                    )?;
                }
                ast::Stmt::IndexAssign {
                    collection,
                    index,
                    expr,
                    ..
                } => {
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
        }

        self.exit_scope(module)?;

        Ok(())
    }

    fn emit_named_assign(
        &mut self,
        name: &str,
        rhs: &Spanned<ast::Expr>,
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
        if let ast::Expr::Var(src_val_name) = &rhs.node {
            if src_val_name == name {
                return Ok(target_ptr);
            }
        }

        let val_ptr = self.compile_owned_expr(rhs, module, "assign_owned")?;

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

        let rhs_ty = self.infer_type(rhs);
        if target.is_annotated && !target.is_ambi {
            if !types_compatible(&target.ty, &rhs_ty) {
                return Err(SprsError::Type {
                    code: ErrorCode {
                        category: ErrorCategory::Type,
                        number: 6,
                    },
                    location: self.location(span),
                    message: format!(
                        "Type mismatch: cannot assign {} to fixed binding `{}` of type {}",
                        rhs_ty, name, target.ty
                    ),
                    expected_type: Some(format!("{}", target.ty)),
                    actual_type: Some(format!("{}", rhs_ty)),
                    help: Some(
                        "use `>> ambi T` if this parameter should allow dynamic reassignment"
                            .to_string(),
                    ),
                });
            }
        }

        let drop_fn = self.get_runtime_fn(module, "__drop")?;
        builder_helper::drop_var(self, target_ptr, drop_fn, name);

        let new_val = self
            .builder
            .build_load(self.runtime_value_type, val_ptr, "assign_load")
            .unwrap();
        self.builder.build_store(target_ptr, new_val).unwrap();

        // Update static type: ambi / unannotated bindings track the RHS.
        if !target.is_annotated || target.is_ambi {
            self.set_variable_type(name, rhs_ty);
        }

        Ok(target_ptr)
    }

    pub(crate) fn compile_expr(
        &mut self,
        expr: &Spanned<ast::Expr>,
        module: &Module<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, SprsError> {
        match &expr.node {
            ast::Expr::Number(number_value) => Ok(builder_helper::create_integer(self, number_value)?),
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
            ast::Expr::Assign(name, rhs) => {
                Ok(self.emit_named_assign(name, rhs, module, expr.span)?.into())
            }
            ast::Expr::Var(ident) => {
                if let Some(binding) = self.get_variables(ident) {
                    Ok(binding.value)
                } else if self.is_visible_atom_def(ident) {
                    Ok(create_atom(
                        self,
                        &LabelName::Static(ident.clone()),
                        module,
                    )?)
                } else {
                    Err(SprsError::Semantic {
                        code: ErrorCode { category: ErrorCategory::Semantic, number: 2 },
                        location: self.location(expr.span),
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
                    "buf_len" => Ok(builder_helper::call_builtin_macro_buf_len(self, args, module)?),
                    "buf_get" => Ok(builder_helper::call_builtin_macro_buf_get(self, args, module)?),
                    "buf_set" => Ok(builder_helper::call_builtin_macro_buf_set(self, args, module)?),
                    "clone" => Ok(builder_helper::call_builtin_macro_clone(self, args, module)?),
                    "move" => Ok(builder_helper::call_builtin_macro_move(self, args, module)?),
                    "raw" => Ok(builder_helper::call_builtin_macro_raw(self, args, module)?),
                    "free" => Ok(builder_helper::call_builtin_macro_free(self, args, module)?),
                    "cast" => Ok(builder_helper::call_builtin_macro_cast(self, args, module)?),
                    "fcast" => Ok(builder_helper::call_builtin_macro_fcast(self, args, module)?),
                    "lshift" => Ok(builder_helper::call_builtin_macro_lshift(self, args, module)?),
                    "rshift" => Ok(builder_helper::call_builtin_macro_rshift(self, args, module)?),
                    "not" => Ok(builder_helper::call_builtin_macro_not(self, args, module)?),
                    "is_error" => Ok(builder_helper::call_builtin_macro_is_error(self, args, module)?),
                    "error_message" => Ok(builder_helper::call_builtin_macro_error_message(self, args, module)?),
                    "attach" => Ok(builder_helper::call_builtin_macro_attach(self, args, module)?),
                    "label_is" => Ok(builder_helper::call_builtin_macro_label_is(self, args, module)?),
                    "label_payload" => Ok(builder_helper::call_builtin_macro_label_payload(self, args, module)?),
                    "label_name" => Ok(builder_helper::call_builtin_macro_label_name(self, args, module)?),
                    "error" => Ok(builder_helper::call_builtin_macro_error(self, args, module)?),
                    _ => Err(SprsError::Semantic {
                        code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
                        location: self.location(expr.span),
                        message: format!("Unknown macro: {}", ident),
                        help: None,
                    }),
                }
            }
            ast::Expr::FieldAccess(lhs, rhs) => {
                let lhs_type = self.infer_type(lhs);

                let struct_name = match lhs_type {
                    Type::Struct(name) => name,
                    _ => {
                        return Err(SprsError::Semantic {
                            code: ErrorCode { category: ErrorCategory::Semantic, number: 2 },
                            location: self.location(lhs.span),
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
            ast::Expr::Match { scrutinee, arms } => {
                Ok(builder_helper::create_match_expr(self, scrutinee, arms, module)?)
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
            ast::Expr::Atom(name) => {
                if let LabelName::Static(static_name) = name {
                    self.resolve_closed_label_member(static_name, expr.span)?;
                }
                Ok(create_atom(self, name, module)?)
            }
            ast::Expr::Label(name, payload) => {
                Ok(create_label(self, name, payload, module)?)
            }
            ast::Expr::AttachSlot(slot_name) => {
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
            ast::Expr::StructInit(struct_name, fields) => Ok(builder_helper::create_struct_init(self, struct_name, fields, module)?),
            ast::Expr::Try(inner_expr) => {
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
            ast::Expr::HeapAlloc(size_expr) => {
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
                        self.context
                            .i32_type()
                            .const_int(Tag::Buffer as u64, false),
                    )
                    .unwrap();
                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                self.builder.build_store(res_data_ptr, handle).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Destroy(inner_expr) => {
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
                        self.context
                            .i32_type()
                            .const_int(Tag::Unit as u64, false),
                    )
                    .unwrap();

                let res_ptr = builder_helper::create_entry_block_alloca(self, "destroy_res_alloc")?;
                self.tag_only_runtime_value_store(res_ptr, Tag::Unit as u64, "destroy_unit");
                Ok(res_ptr.into())
            }
            ast::Expr::Exist(inner_expr) => {
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
                let cont_bb = self
                    .context
                    .append_basic_block(current_fn, "exist_cont_bb");

                let res_ptr =
                    builder_helper::create_entry_block_alloca(self, "exist_res_alloc")?;

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
                self.builder
                    .build_unconditional_branch(cont_bb)
                    .unwrap();

                self.builder.position_at_end(false_bb);
                self.tag_only_runtime_value_store(res_ptr, Tag::Boolean as u64, "exist_false_unit");
                let false_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "exist_res_data_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        false_data_ptr,
                        self.context.i64_type().const_int(0, false),
                    )
                    .unwrap();
                self.builder
                    .build_unconditional_branch(cont_bb)
                    .unwrap();

                self.builder.position_at_end(cont_bb);
                Ok(res_ptr.into())
            }
        }
    }
}

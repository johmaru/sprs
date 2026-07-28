use crate::front::ast;
use crate::front::span::Spanned;
use crate::front::span::Span;
use crate::front::type_helper;
use crate::front::type_helper::Type;
use crate::llvm::builder_helper;
use crate::llvm::builder_helper::Comparison;
use crate::llvm::builder_helper::EqNeq;
use crate::llvm::builder_helper::UpDown;
use inkwell::AddressSpace;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use crate::llvm::compiler::{Compiler, OS, Tag};
use crate::naming;

impl<'ctx> Compiler<'ctx> {
    pub fn get_known_type_from_expr(&self, expr: &Spanned<ast::Expr>) -> Result<String, String> {
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
            ast::Expr::Float(_) => Ok("default(f64)".to_string()),
            _ => Err(format!(
                "Unknown type expression for known type: {:?}",
                expr
            )),
        }
    }

    pub fn get_expr_name(&self, expr: &Spanned<ast::Expr>) -> Option<String> {
        match &expr.node {
            ast::Expr::Var(name) => Some(name.clone()),
            _ => None,
        }
    }

    fn infer_type(&self, expr: &Spanned<ast::Expr>) -> Type {
        match &expr.node {
            ast::Expr::Number(_) => Type::Int,
            ast::Expr::Float(_) => Type::Float,
            ast::Expr::Str(_) => Type::Str,
            ast::Expr::Bool(_) => Type::Bool,
            ast::Expr::Unit() => Type::Unit,
            ast::Expr::Var(name) => self
                .get_variables(name)
                .map(|(_, ty)| ty.clone())
                .unwrap_or(Type::Any),
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
            ast::Expr::Increment(value) | ast::Expr::Decrement(value) | ast::Expr::Neg(value) => self.infer_type(value),
            ast::Expr::If(_, then, if_else) => {
                let then_ty = self.infer_type(then);
                let else_ty = self.infer_type(if_else);
                if then_ty == else_ty {
                    then_ty
                } else {
                    Type::Any
                }
            }
            ast::Expr::Call(_, _, ret_ty_opt) => {
                if let Some(ret_ty) = ret_ty_opt {
                    ret_ty.clone()
                } else {
                    Type::Any
                }
            }
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
                "init" => Type::Any,
                _ => Type::Any,
            },
            ast::Expr::StructInit(name, _) => Type::Struct(name.clone()),
            _ => Type::Any,
        }
    }

    pub fn compile_fn(
        &mut self,
        func: &ast::Function,
        module: &Module<'ctx>,
    ) -> Result<FunctionValue<'ctx>, String> {
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
            .ok_or_else(|| format!("Function {} not declared", func_name))?;

        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);
        self.function_signatures = Some(fn_val);

        self.enter_scope();

        for (idx, param) in func.params.iter().enumerate() {
            let arg_val = fn_val.get_nth_param(idx as u32).unwrap();

            let alloca = self
                .builder
                .build_alloca(self.runtime_value_type, &param.ident)
                .unwrap();
            self.builder
                .build_store(alloca, arg_val)
                .map_err(|e| e.to_string())?;
            self.add_variable(param.ident.clone(), alloca.into(), Type::Any);
        }

        self.compile_block(&func.blk, module)?;

        let current_block = self.builder.get_insert_block().unwrap();
        if current_block.get_terminator().is_none() {
            // Inter compile_block will execute exit_scope, so need scope of function args end here
            self.exit_scope(module)?;
            builder_helper::create_dummy_for_no_return(self);
        } else {
            self.scopes.pop();
        }

        if fn_val.verify(true) {
            Ok(fn_val)
        } else {
            unsafe {
                fn_val.delete();
            }
            Err("Invalid generated function".to_string())
        }
    }

    /// Process a `return` statement: type-check the expression, convert it
    /// to the function's return type, emit drops, and build the `ret` instr.
    fn compile_return(
        &mut self,
        expr_opt: &Option<Spanned<ast::Expr>>,
        module: &Module<'ctx>,
    ) -> Result<(), String> {
        let ret_val = if let Some(expr) = expr_opt {
            let ptr = self.compile_expr(expr, module)?.into_pointer_value();

            if let ast::Expr::Var(name) = &expr.node {
                let var_val = self.get_variables(name).map(|(v, _)| v);
                if let Some(val) = var_val {
                    let val_ptr = val.into_pointer_value();
                    builder_helper::var_return_store(self, &val_ptr.into(), name);
                }
            }

            let current_fn = self.function_signatures.unwrap();
            let return_type = current_fn.get_type().get_return_type();
            let expr_type = self.infer_type(expr);

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

    /// Validate that the expression type matches the function return type.
    fn validate_return_type(
        &self,
        return_type: Option<BasicTypeEnum<'ctx>>,
        expr_type: Type,
        expr: &Spanned<ast::Expr>,
    ) -> Result<(), String> {
        if let Some(ret_ty) = return_type {
            if ret_ty.is_pointer_type() {
                let llvm_int_ty = type_helper::is_int_type_in_llvm();
                if llvm_int_ty.contains(&expr_type) {
                    return Err(format!(
                        "Type mismatch: Function expects pointer type (e.g. str) but got {:?} from expression {:?}",
                        expr_type, expr
                    ));
                }
            } else if ret_ty.is_int_type() {
                let width = ret_ty.into_int_type().get_bit_width();
                if width == 1 {
                    if expr_type != Type::Bool {
                        return Err(format!(
                            "Type mismatch: Function expects Bool but got {:?} from expression {:?}",
                            expr_type, expr
                        ));
                    }
                } else {
                    let llvm_not_int = type_helper::not_int_type_in_llvm();
                    if llvm_not_int.contains(&expr_type) {
                        return Err(format!(
                            "Type mismatch: Function expects Int type but got {:?} from expression {:?}",
                            expr_type, expr
                        ));
                    }
                }
            } else if ret_ty.is_float_type() {
                let llvm_float_ty = type_helper::is_float_type_in_llvm();
                if !llvm_float_ty.contains(&expr_type) {
                    return Err(format!(
                        "Type mismatch: Function expects Float type but got {:?} from expression {:?}",
                        expr_type, expr
                    ));
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
    ) -> Result<Option<BasicValueEnum<'ctx>>, String> {
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
                    return Err("Unsupported return type conversion".to_string());
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
    ) -> Result<(), String> {
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
                    let init_val = self
                        .compile_expr(init_expr, module)?
                        .into_pointer_value();

                    let var_type = self.infer_type(init_expr);

                    builder_helper::var_load_at_init_variable(self, init_val, &var.ident)?;

                    if let Some(expr) = &var.expr {
                        if let ast::Expr::Var(src_val_name) = &expr.node {
                            let var_val = self.get_variables(src_val_name).map(|(v, _)| v);
                            if let Some(val) = var_val {
                                builder_helper::move_variable(self, &val, &var.ident);
                            }
                        }
                    }
                    self.add_variable(var.ident.clone(), init_val.into(), var_type);
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
                    let val_ptr = self
                        .compile_expr(&assign_stmt.expr, module)?
                        .into_pointer_value();

                    let (target_val, _) = self
                        .get_variables(&assign_stmt.name)
                        .ok_or_else(|| format!("Undefined variable: {}", &assign_stmt.name))?;

                    let target_ptr = target_val.into_pointer_value();

                    let drop_fn = self.get_runtime_fn(module, "__drop")?;
                    builder_helper::drop_var(self, target_ptr, drop_fn, &assign_stmt.name);

                    let new_val = self
                        .builder
                        .build_load(self.runtime_value_type, val_ptr, "assign_load")
                        .unwrap();
                    self.builder
                        .build_store(target_ptr, new_val)
                        .map_err(|e| e.to_string())?;

                    if let ast::Expr::Var(src_val_name) = &assign_stmt.expr.node {
                        let var_val = self.get_variables(src_val_name).map(|(v, _)| v);
                        if let Some(val) = var_val {
                            builder_helper::move_variable(self, &val, &assign_stmt.name);
                        }
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
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match &expr.node {
            ast::Expr::Number(n) => {
                let result = builder_helper::create_integer(self, n);
                result
            }
            ast::Expr::Float(fp) => {
                let result = builder_helper::create_float(self, *fp);
                result
            }
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
            ast::Expr::Str(str) => {
                let result = builder_helper::create_string(self, str, module);
                result
            }
            ast::Expr::Bool(boolean) => {
                let result = builder_helper::create_bool(self, boolean);
                result
            }
            ast::Expr::Var(ident) => {
                if let Some((var_addr, _)) = self.get_variables(ident) {
                    Ok(var_addr)
                } else {
                    Err(format!("Undefined variable: {}", ident))
                }
            }
            ast::Expr::Call(ident, args, _) => {
                let result = builder_helper::create_call_expr(self, ident, args, module);
                result
            }
            ast::Expr::Macro(ident, args) => {
                match ident.as_str() {
                    "println" => builder_helper::call_builtin_macro_println(self, args, module),
                    "list_push" => builder_helper::call_builtin_macro_list_push(self, args, module),
                    "clone" => builder_helper::call_builtin_macro_clone(self, args, module),
                    "cast" => builder_helper::call_builtin_macro_cast(self, args, module),
                    "lshift" => builder_helper::call_builtin_macro_lshift(self, args, module),
                    "rshift" => builder_helper::call_builtin_macro_rshift(self, args, module),
                    "not" => builder_helper::call_builtin_macro_not(self, args, module),
                    "init" => Err("struct initialization requires @init(TypeName { field: value, ... }) syntax".to_string()),
                    _ => Err(format!("Unknown macro: {}", ident)),
                }
            }
            ast::Expr::FieldAccess(lhs, rhs) => {
                if let ast::Expr::Var(name) = &lhs.node {
                    if self.enum_names.contains(name) {
                        let full_name = format!("{}.{}", name, rhs);
                        if let Some((var_addr, _)) = self.get_variables(&full_name) {
                            return Ok(var_addr);
                        } else {
                            return Err(format!("Undefined enum variant: {}", full_name));
                        }
                    }
                }

                let lhs_type = self.infer_type(lhs);

                let struct_name = match lhs_type {
                    Type::Struct(name) => name,
                    _ => {
                        return Err(format!(
                            "Undefined variable: {}",
                            self.get_expr_name(lhs).unwrap_or_default()
                        ));
                    }
                };

                let index = self.get_field_index(&struct_name, rhs)?;

                let result =
                    builder_helper::create_field_access(self, lhs, index, &struct_name, module);
                result
            }
            ast::Expr::Add(lhs, rhs) => {
                let result = builder_helper::create_add_expr(self, lhs, rhs, module);
                result
            }
            ast::Expr::Mul(lhs, rhs) => {
                let result = builder_helper::create_mul_expr(self, lhs, rhs, module);
                result
            }
            ast::Expr::Minus(lhs, rhs) => {
                let result = builder_helper::create_minus_expr(self, lhs, rhs, module);
                result
            }
            ast::Expr::Div(lhs, rhs) => {
                let result = builder_helper::create_div_expr(self, lhs, rhs, module);
                result
            }
            ast::Expr::Mod(lhs, rhs) => {
                let result = builder_helper::create_mod_expr(self, lhs, rhs, module);
                result
            }
            ast::Expr::Increment(expr) => {
                let result =
                    builder_helper::create_increment_or_decrement(self, expr, UpDown::Up, module);
                result
            }
            ast::Expr::Decrement(expr) => {
                let result =
                    builder_helper::create_increment_or_decrement(self, expr, UpDown::Down, module);
                result
            }
            ast::Expr::Neg(expr) => {
                let zero = Spanned::new(ast::Expr::Number(0), Span::DUMMY);
                let result = builder_helper::create_minus_expr(
                    self,
                    &zero,
                    expr,
                    module,
                );
                result
            }
            ast::Expr::Eq(lhs, rhs) => {
                let result = builder_helper::create_eq_or_neq(
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
                );
                result
            }
            ast::Expr::Neq(lhs, rhs) => {
                let result = builder_helper::create_eq_or_neq(
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
                );
                result
            }
            ast::Expr::Gt(lhs, rhs) => {
                let result = builder_helper::create_comparison(
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
                );
                result
            }
            ast::Expr::Lt(lhs, rhs) => {
                let result = builder_helper::create_comparison(
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
                );
                result
            }
            ast::Expr::Ge(lhs, rhs) => {
                let result = builder_helper::create_comparison(
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
                );
                result
            }
            ast::Expr::Le(lhs, rhs) => {
                let result = builder_helper::create_comparison(
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
                );
                result
            }
            ast::Expr::If(cond, then_expr, else_expr) => {
                let result =
                    builder_helper::create_if_expr(self, cond, then_expr, else_expr, module);
                result
            }
            ast::Expr::List(elements) => {
                let result = builder_helper::create_list(self, elements, module);
                result
            }
            ast::Expr::Index(collection_expr, index_expr) => {
                let result =
                    builder_helper::create_index(self, collection_expr, index_expr, module);
                result
            }
            ast::Expr::Range(start_expr, end_expr) => {
                let result = builder_helper::create_range(self, start_expr, end_expr, module);
                result
            }
            ast::Expr::ModuleAccess(module_name, function_name, args) => {
                let result = builder_helper::create_module_access(
                    self,
                    module_name,
                    function_name,
                    args,
                    module,
                );
                result
            }
            ast::Expr::Unit() => {
                let result = builder_helper::create_unit(self);
                result
            }
            ast::Expr::StructInit(struct_name, fields) => {
                let result = builder_helper::create_struct_init(self, struct_name, fields, module);
                result
            }
        }
    }
}

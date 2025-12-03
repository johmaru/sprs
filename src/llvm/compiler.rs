use crate::command_helper;
use crate::front::ast;
use crate::interpreter::runner::parse_only;
use crate::llvm::builder_helper;
use crate::llvm::builder_helper::Comparison;
use crate::llvm::builder_helper::EqNeq;
use crate::llvm::builder_helper::UpDown;
use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, StructType};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue, ValueKind};
use std::collections::HashMap;
use std::result;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub modules: HashMap<String, Module<'ctx>>, // name, module
    pub builder: Builder<'ctx>,
    pub variables: HashMap<String, BasicValueEnum<'ctx>>,
    pub function_signatures: Option<FunctionValue<'ctx>>,
    pub runtime_value_type: StructType<'ctx>,
    pub target_os: OS,
    pub string_constants: HashMap<String, inkwell::values::GlobalValue<'ctx>>,
    pub malloc_type: inkwell::types::FunctionType<'ctx>,
    pub source_path: String,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum OS {
    Unknown, // default triple
    Windows,
    Linux,
}

pub enum Tag {
    Integer = 0,
    String = 1,
    Boolean = 2,
    List = 3,
    Range = 4,
    Unit = 5,
}

const WINDOWS_STR: &str = "Windows";
const LINUX_STR: &str = "Linux";

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, builder: Builder<'ctx>, source_path: String) -> Self {
        let runtime_value_type = context.struct_type(
            &[context.i32_type().into(), context.i64_type().into()],
            false,
        );

        let i64_type = context.i64_type();
        let i8_ptr_type = context.ptr_type(AddressSpace::default());
        let malloc_type = i8_ptr_type.fn_type(&[i64_type.into()], false);

        Compiler {
            context,
            modules: HashMap::new(),
            builder,
            variables: HashMap::new(),
            function_signatures: None,
            runtime_value_type,
            target_os: OS::Unknown,
            string_constants: HashMap::new(),
            malloc_type,
            source_path,
        }
    }

    pub fn build_list_from_exprs(
        &mut self,
        elements: &[ast::Expr],
        module: &Module<'ctx>,
    ) -> Result<PointerValue<'ctx>, String> {
        let create = builder_helper::create_list_from_expr(self, elements, module);
        create
    }

    pub fn get_runtime_fn(&self, module: &Module<'ctx>, name: &str) -> FunctionValue<'ctx> {
        if let Some(func) = module.get_function(name) {
            return func;
        }

        let i64_type = self.context.i64_type();
        let i32_type = self.context.i32_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let void_type = self.context.void_type();

        let fn_type = match name {
            "__list_new" => i8_ptr_type.fn_type(&[i64_type.into()], false),
            "__list_push" => void_type.fn_type(
                &[
                    i8_ptr_type.into(), // list ptr
                    i32_type.into(),    // value tag
                    i64_type.into(),    // value data
                ],
                false,
            ),
            "__list_get" => i8_ptr_type.fn_type(
                &[
                    i8_ptr_type.into(), // list ptr
                    i64_type.into(),    // index
                ],
                false,
            ),
            "__range_new" => i8_ptr_type.fn_type(
                &[
                    i64_type.into(), // start
                    i64_type.into(), // end
                ],
                false,
            ),
            "__println" => void_type.fn_type(&[i8_ptr_type.into()], false),
            "__strlen" => i64_type.fn_type(&[i8_ptr_type.into()], false),
            "__malloc" => i8_ptr_type.fn_type(&[i64_type.into()], false),
            "__drop" => void_type.fn_type(&[i32_type.into(), i64_type.into()], false),
            "__clone" => self.runtime_value_type.fn_type(
                &[
                    i32_type.into(), // value tag
                    i64_type.into(), // value data
                ],
                false,
            ),
            _ => panic!("Unknown runtime function: {}", name),
        };

        module.add_function(name, fn_type, None)
    }

    pub fn load_and_compile_module(
        &mut self,
        module_name: &str,
        main_path: Option<&String>,
    ) -> Result<(), String> {
        if self.modules.contains_key(module_name) {
            return Ok(());
        }

        let mut path = format!("{}/{}.sprs", self.source_path, module_name);

        if let Some(main_path) = main_path {
            if module_name == "main" {
                path = main_path.clone();
            }
        }

        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read module file {}: {}", path, e))?;

        let items = parse_only(&source)?;

        self.process_preprocessors(&items);

        let llvm_module_name = items
            .iter()
            .find_map(|item| match item {
                ast::Item::Package(name) => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| module_name.to_string());

        let module = self.context.create_module(&llvm_module_name);

        self.inject_runtime_constants(&module);

        for item in &items {
            if let ast::Item::Import(import_name) = item {
                self.load_and_compile_module(import_name, None)?;
            }
        }

        for item in &items {
            match item {
                ast::Item::FunctionItem(func) => {
                    self.declare_fn_prototype(func, &module);
                }
                _ => {}
            }
        }

        for item in &items {
            match item {
                ast::Item::FunctionItem(func) => {
                    self.compile_fn(func, &module)?;
                }
                _ => {}
            }
        }

        if llvm_module_name == "main" {
            if let Some(sprs_main_fn) = module.get_function("_sprs_main") {
                let i32_type = self.context.i32_type();
                let main_type = i32_type.fn_type(&[], false);
                let c_main = module.add_function("main", main_type, None);

                let entry = self.context.append_basic_block(c_main, "entry");
                self.builder.position_at_end(entry);

                self.builder
                    .build_call(sprs_main_fn, &[], "call_sprs_main")
                    .unwrap();

                self.builder
                    .build_return(Some(&i32_type.const_int(0, false)))
                    .unwrap();
            }
        }

        self.modules.insert(llvm_module_name, module);
        Ok(())
    }

    fn process_preprocessors(&mut self, items: &Vec<ast::Item>) {
        for item in items {
            if let ast::Item::Preprocessor(pre) = item {
                if pre.starts_with("Windows") {
                    self.target_os = OS::Windows;
                } else if pre.starts_with("Linux") {
                    self.target_os = OS::Linux;
                }
            }
        }
    }

    fn inject_runtime_constants(&self, module: &Module<'ctx>) {
        let os_str = match self.target_os {
            OS::Unknown => "Unknown",
            OS::Windows => WINDOWS_STR,
            OS::Linux => LINUX_STR,
        };
        let os_str_val = self.context.const_string(os_str.as_bytes(), true);

        let global = module.add_global(
            os_str_val.get_type(),
            Some(AddressSpace::default()),
            "TARGET_OS",
        );
        global.set_initializer(&os_str_val);
        global.set_linkage(Linkage::Internal);
        global.set_constant(true);
    }

    fn declare_fn_prototype(&self, func: &ast::Function, module: &Module<'ctx>) {
        let arg_types: Vec<BasicMetadataTypeEnum> = (0..func.params.len())
            .map(|_| self.context.ptr_type(AddressSpace::default()).into())
            .collect();

        let fn_type = self.runtime_value_type.fn_type(&arg_types, false);

        let func_name = if func.ident == "main" {
            "_sprs_main"
        } else {
            &func.ident
        };

        if module.get_function(func_name).is_none() {
            module.add_function(func_name, fn_type, None);
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

        let fn_type = self.runtime_value_type.fn_type(&arg_types, false);

        let func_name = if func.ident == "main" {
            "_sprs_main"
        } else {
            &func.ident
        };

        let fn_val = if let Some(f) = module.get_function(func_name) {
            f
        } else {
            module.add_function(func_name, fn_type, None)
        };

        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);
        self.function_signatures = Some(fn_val);

        for (idx, param) in func.params.iter().enumerate() {
            let arg_val = fn_val.get_nth_param(idx as u32).unwrap();

            let alloca = self
                .builder
                .build_alloca(self.runtime_value_type, &param.ident)
                .unwrap();
            let _ = self.builder.build_store(alloca, arg_val);
            self.variables.insert(param.ident.clone(), alloca.into());
        }

        self.compile_block(&func.blk, module)?;

        let current_block = self.builder.get_insert_block().unwrap();
        if current_block.get_terminator().is_none() {
            builder_helper::create_dummy_for_no_return(self);
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

    pub(crate) fn compile_block(
        &mut self,
        stmts: &Vec<ast::Stmt>,
        module: &Module<'ctx>,
    ) -> Result<(), String> {
        let mut local_vars: Vec<String> = Vec::new();

        for stmt in stmts {
            match stmt {
                ast::Stmt::Var(var) => {
                    let init_val = self.compile_expr(&var.expr.as_ref().unwrap_or(&ast::Expr::Unit()), module)?.into_pointer_value();

                    if let Some(&existing) = self.variables.get(&var.ident) {
                        let ptr = existing.into_pointer_value();

                        builder_helper::load_at_init_variable_with_existing(self, init_val, ptr, &var.ident);

                        if let Some(ast::Expr::Var(src_val_name)) = &var.expr {
                            if let Some(&src_ptr_enum) = self.variables.get(src_val_name) {
                                builder_helper::move_variable(self, &src_ptr_enum, &var.ident);
                            }
                        }
                        local_vars.push(var.ident.clone());
                    } else {
                        let ptr = builder_helper::var_load_at_init_variable(self, init_val, &var.ident);
                        if let Some(ast::Expr::Var(src_val_name)) = &var.expr {
                            if let Some(&src_ptr_enum) = self.variables.get(src_val_name) {
                                builder_helper::move_variable(self, &src_ptr_enum, &var.ident);
                            }
                        }
                        local_vars.push(var.ident.clone());
                        self.variables.insert(var.ident.clone(), ptr.into());
                    }
                }
                ast::Stmt::Return(expr_opt) => {
                    let ret_val = if let Some(expr) = expr_opt {
                        let ptr = self.compile_expr(expr, module)?.into_pointer_value();

                        if let ast::Expr::Var(name) = expr {
                            if let Some(&var_ptr_enum) = self.variables.get(name) {
                                builder_helper::var_return_store(self, &var_ptr_enum, name);
                            }
                        }

                        let val = self
                            .builder
                            .build_load(self.runtime_value_type, ptr, "return_load")
                            .unwrap();
                        Some(val)
                    } else {
                        None
                    };
                    let drop_fn = self.get_runtime_fn(module, "__drop");

                    if self
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_terminator()
                        .is_none()
                    {
                        for var_name in local_vars.iter().rev() {
                            if let Some(val_enum) = self.variables.get(var_name) {
                                let ptr = val_enum.into_pointer_value();

                                builder_helper::drop_var(self, ptr, drop_fn, var_name);
                            }
                        }
                    }

                    if let Some(val) = ret_val {
                        self.builder.build_return(Some(&val)).unwrap();
                    } else {
                        builder_helper::create_dummy_for_no_return(self);
                    }
                }
                ast::Stmt::If {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    let _ = builder_helper::create_if_condition(self, cond, then_blk, else_blk, module);
                }
                ast::Stmt::While { cond, body } => {
                    let _ = builder_helper::create_while_condition(self, cond, body, module);
                }
                ast::Stmt::Expr(expr) => {
                    self.compile_expr(expr, module)?;
                }
            }
        }
        let drop_fn = self.get_runtime_fn(module, "__drop");
        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            for var_name in local_vars.iter().rev() {
                if let Some(val_enum) = self.variables.get(var_name) {
                    let ptr = val_enum.into_pointer_value();

                    builder_helper::drop_var(self, ptr, drop_fn, var_name);
                }
            }
        }

        Ok(())
    }

    pub(crate) fn compile_expr(
        &mut self,
        expr: &ast::Expr,
        module: &Module<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match expr {
            ast::Expr::Number(n) => {
                let result = builder_helper::create_integer(self, n);
                result
            }
            ast::Expr::Str(str) => {
                let result = builder_helper::create_string(self, str, module);
                result
            }
            ast::Expr::Bool(boolean) => {
                let result = builder_helper::create_bool(self, boolean);
                result
            }
            ast::Expr::Var(ident) => {
                if let Some(var_addr) = self.variables.get(ident) {
                    Ok(*var_addr)
                } else {
                    Err(format!("Undefined variable: {}", ident))
                }
            }
            ast::Expr::Call(ident, args, _) => {
                if ident == "println!" {
                    let result = builder_helper::call_builtin_macro_println(self, args, module);
                    return result;
                }

                if ident == "list_push!" {
                    let result = builder_helper::call_builtin_macro_list_push(self, args, module);
                    return result;
                }

                if ident == "clone!" {
                    let result = builder_helper::call_builtin_macro_clone(self, args, module);
                    return result;
                }

                let result = builder_helper::create_call_expr(self, ident, args, module);
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
                let result = builder_helper::create_increment_or_decrement(self, expr, UpDown::Up, module);
                result
            }
            ast::Expr::Decrement(expr) => {
                let result = builder_helper::create_increment_or_decrement(self, expr, UpDown::Down, module);
                result
            }
            ast::Expr::Eq(lhs, rhs) => {
                let result = builder_helper::create_eq_or_neq(self, lhs, rhs, module, EqNeq::Eq,|builder, l_val, r_val, name| {
                    Ok(builder.build_int_compare(inkwell::IntPredicate::EQ, l_val, r_val, name).unwrap())
                });
                result
            }
            ast::Expr::Neq(lhs, rhs) => {
                let result = builder_helper::create_eq_or_neq(self, lhs, rhs, module, EqNeq::Neq,|builder, l_val, r_val, name| {
                    Ok(builder.build_int_compare(inkwell::IntPredicate::NE, l_val, r_val, name).unwrap())
                });
                result
            }
            ast::Expr::Gt(lhs, rhs) => {
                let result = builder_helper::create_comparison(self, lhs, rhs, module, Comparison::Gt, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_compare(inkwell::IntPredicate::SGT, l_val, r_val, name).unwrap())
                });
                result
            }
            ast::Expr::Lt(lhs, rhs) => {
                let result = builder_helper::create_comparison(self, lhs, rhs, module, Comparison::Lt, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_compare(inkwell::IntPredicate::SLT, l_val, r_val, name).unwrap())
                });
                result
            }
            ast::Expr::Ge(lhs, rhs) => {
                let result = builder_helper::create_comparison(self, lhs, rhs, module, Comparison::Ge, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_compare(inkwell::IntPredicate::SGE, l_val, r_val, name).unwrap())
                });
                result
            }
            ast::Expr::Le(lhs, rhs) => {
               let result = builder_helper::create_comparison(self, lhs, rhs, module, Comparison::Le, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_compare(inkwell::IntPredicate::SLE, l_val, r_val, name).unwrap())
                });
                result
            }
            ast::Expr::If(cond, then_expr, else_expr) => {
                let result = builder_helper::create_if_expr(self, cond, then_expr, else_expr, module);
                result
            }
            ast::Expr::List(elements) => {
                let result = builder_helper::create_list(self, elements, module);
                result
            }
            ast::Expr::Index(collection_expr, index_expr) => {
                let result = builder_helper::create_index(self, collection_expr, index_expr, module);
                result
            }
            ast::Expr::Range(start_expr, end_expr) => {
                let result = builder_helper::create_range(self, start_expr, end_expr, module);
                result
            }
            ast::Expr::ModuleAccess(module_name, function_name, args) => {
                let result = builder_helper::create_module_access(self, module_name, function_name, args, module);
                result
            },
            ast::Expr::Unit() => {
                let result = builder_helper::create_unit(self);
                result
            }
        }
    }
}

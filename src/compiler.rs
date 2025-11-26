use crate::ast;
use crate::runner::parse_only;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue};
use std::collections::HashMap;

pub struct Compiler<'a, 'ctx> {
    pub context: &'ctx Context,
    pub modules: HashMap<String, Module<'ctx>>, // name, module
    pub builder: Builder<'ctx>,
    pub variables: HashMap<String, BasicValueEnum<'ctx>>,
    pub function_signatures: Option<FunctionValue<'ctx>>,
}

impl<'a, 'ctx> Compiler<'a, 'ctx> {
    pub fn compile_fn(
        &mut self,
        func: &ast::Function,
        module: &Module<'ctx>,
    ) -> Result<FunctionValue<'ctx>, String> {
        let ret_type = self.context.i64_type();
        let fn_type = ret_type.fn_type(&[], false);
        let fn_val = module.add_function(&func.ident, fn_type, None);

        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);
        self.function_signatures = Some(fn_val);

        self.compile_block(&func.blk)?;

        if fn_val.verify(true) {
            Ok(fn_val)
        } else {
            unsafe {
                fn_val.delete();
            }
            Err("Invalid generated function".to_string())
        }
    }

    pub fn load_and_compile_module(&mut self, module_name: &str) -> Result<(), String> {
        if self.modules.contains_key(module_name) {
            return Ok(());
        }

        let path = format!("{}.sprs", module_name);
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read module file {}: {}", path, e))?;

        let items = parse_only(&source)?;

        let llvm_module_name = items
            .iter()
            .find_map(|item| match item {
                ast::Item::Package(name) => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| module_name.to_string());

        let module = self.context.create_module(&llvm_module_name);

        for item in &items {
            if let ast::Item::Import(import_name) = item {
                self.load_and_compile_module(import_name)?;
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

        self.modules.insert(llvm_module_name, module);
        Ok(())
    }

    fn compile_block(&mut self, stmts: &Vec<ast::Stmt>) -> Result<(), String> {
        for stmt in stmts {
            match stmt {
                ast::Stmt::Return(expr_opt) => {
                    if let Some(expr) = expr_opt {
                        let val = self.compile_expr(expr)?;
                        self.builder.build_return(Some(&val));
                    } else {
                        self.builder.build_return(None);
                    }
                }
                ast::Stmt::Expr(expr) => {
                    self.compile_expr(expr)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn compile_module(&mut self, items: &Vec<ast::Item>, filename: &str) -> Result<(), String> {
        let module_name = items
            .iter()
            .find_map(|item| match item {
                ast::Item::Package(name) => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| filename.replace(".sprs", ""));

        let module = self.context.create_module(&module_name);

        for item in items {
            match item {
                ast::Item::FunctionItem(func) => {
                    self.compile_fn(func, &module)?;
                }
                _ => {}
            }
        }

        self.modules.insert(module_name, module);
        Ok(())
    }

    fn compile_expr(&mut self, expr: &ast::Expr) -> Result<BasicValueEnum<'ctx>, String> {
        match expr {
            ast::Expr::Number(n) => Ok(self.context.i64_type().const_int(*n as u64, false).into()),
            ast::Expr::Add(lhs, rhs) => {
                let l = self.compile_expr(lhs)?.into_int_value();
                let r = self.compile_expr(rhs)?.into_int_value();
                Ok(self.builder.build_int_add(l, r, "addtmp").into())
            }
            _ => Err("Not implemented".to_string()),
        }
    }
}

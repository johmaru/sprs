use crate::ast;
use crate::runner::parse_only;
use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, StructType};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue, ValueKind};
use std::collections::HashMap;
use std::i8;

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
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum OS {
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
    pub fn new(context: &'ctx Context, builder: Builder<'ctx>) -> Self {
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
            target_os: if cfg!(target_os = "windows") {
                OS::Windows
            } else {
                OS::Linux
            },
            string_constants: HashMap::new(),
            malloc_type,
        }
    }

    pub fn build_list_from_exprs(
        &mut self,
        elements: &[ast::Expr],
        module: &Module<'ctx>,
    ) -> Result<PointerValue<'ctx>, String> {
        let len = elements.len();
        let i64_type = self.context.i64_type();

        let list_new_fn = self.get_runtime_fn(module, "__list_new");

        let list_ptr = self
            .builder
            .build_call(
                list_new_fn,
                &[i64_type.const_int(len as u64, false).into()],
                "list_ptr",
            )
            .unwrap();

        let list_ptr_val = match list_ptr.try_as_basic_value() {
            ValueKind::Basic(val) => val.into_pointer_value(),
            _ => return Err("Expected a basic value".to_string()),
        };

        let list_push_fn = self.get_runtime_fn(module, "__list_push");

        for elem in elements {
            let val_ptr = self.compile_expr(elem, module)?.into_pointer_value();

            let target_ptr = self
                .builder
                .build_struct_gep(self.runtime_value_type, val_ptr, 0, "val_tag_ptr")
                .unwrap();
            let val_tag = self
                .builder
                .build_load(self.context.i32_type(), target_ptr, "val_tag")
                .unwrap()
                .into_int_value();

            let data_ptr = self
                .builder
                .build_struct_gep(self.runtime_value_type, val_ptr, 1, "val_data_ptr")
                .unwrap();
            let val_data = self
                .builder
                .build_load(self.context.i64_type(), data_ptr, "val_data")
                .unwrap()
                .into_int_value();

            self.builder
                .build_call(
                    list_push_fn,
                    &[list_ptr_val.into(), val_tag.into(), val_data.into()],
                    "list_push_call",
                )
                .unwrap();
        }
        Ok(list_ptr_val)
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
            _ => panic!("Unknown runtime function: {}", name),
        };

        module.add_function(name, fn_type, None)
    }

    pub fn load_and_compile_module(&mut self, module_name: &str) -> Result<(), String> {
        if self.modules.contains_key(module_name) {
            return Ok(());
        }

        let path = format!("{}.sprs", module_name);
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

    pub fn compile_module(&mut self, items: &Vec<ast::Item>, filename: &str) -> Result<(), String> {
        let module_name = items
            .iter()
            .find_map(|item| match item {
                ast::Item::Package(name) => Some(name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| filename.replace(".sprs", ""));

        let module = self.context.create_module(&module_name);

        self.process_preprocessors(&items);

        self.inject_runtime_constants(&module);

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

    pub fn compile_fn(
        &mut self,
        func: &ast::Function,
        module: &Module<'ctx>,
    ) -> Result<FunctionValue<'ctx>, String> {
        let arg_types: Vec<BasicMetadataTypeEnum> = (0..func.params.len())
            .map(|_| self.context.ptr_type(AddressSpace::default()).into())
            .collect();

        let fn_type = self.runtime_value_type.fn_type(&arg_types, false);
        let fn_val = module.add_function(&func.ident, fn_type, None);

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
            let dummy_alloca = self
                .builder
                .build_alloca(self.runtime_value_type, "end_dummy")
                .unwrap();

            let tag_ptr = self
                .builder
                .build_struct_gep(self.runtime_value_type, dummy_alloca, 0, "end_dummy_tag")
                .unwrap();
            self.builder
                .build_store(
                    tag_ptr,
                    self.context.i32_type().const_int(Tag::Unit as u64, false),
                )
                .unwrap();

            let data_ptr = self
                .builder
                .build_struct_gep(self.runtime_value_type, dummy_alloca, 1, "end_dummy_data")
                .unwrap();
            self.builder
                .build_store(data_ptr, self.context.i64_type().const_int(0, false))
                .unwrap();

            let val = self
                .builder
                .build_load(self.runtime_value_type, dummy_alloca, "end_dummy_val")
                .unwrap();
            self.builder.build_return(Some(&val)).unwrap();
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

    fn compile_block(
        &mut self,
        stmts: &Vec<ast::Stmt>,
        module: &Module<'ctx>,
    ) -> Result<(), String> {
        for stmt in stmts {
            match stmt {
                ast::Stmt::Var(var) => {
                    let init_val = self.compile_expr(&var.expr, module)?.into_pointer_value();

                    if let Some(existing) = self.variables.get(&var.ident) {
                        let ptr = existing.into_pointer_value();

                        let val = self
                            .builder
                            .build_load(self.runtime_value_type, init_val, "assign_load")
                            .unwrap();
                        let _ = self.builder.build_store(ptr, val);
                    } else {
                        let ptr = self
                            .builder
                            .build_alloca(self.runtime_value_type, &var.ident)
                            .unwrap();

                        let val = self
                            .builder
                            .build_load(self.runtime_value_type, init_val, "var_load")
                            .unwrap();
                        let _ = self.builder.build_store(ptr, val).unwrap();

                        self.variables.insert(var.ident.clone(), ptr.into());
                    }
                }
                ast::Stmt::Return(expr_opt) => {
                    if let Some(expr) = expr_opt {
                        let ptr = self.compile_expr(expr, module)?.into_pointer_value();

                        let val = self
                            .builder
                            .build_load(self.runtime_value_type, ptr, "return_val")
                            .unwrap();
                        let _ = self.builder.build_return(Some(&val)).unwrap();
                    } else {
                        let dummy_alloca = self
                            .builder
                            .build_alloca(self.runtime_value_type, "dummy")
                            .unwrap();

                        let tag_ptr = self
                            .builder
                            .build_struct_gep(
                                self.runtime_value_type,
                                dummy_alloca,
                                0,
                                "dummy_tag_ptr",
                            )
                            .unwrap();
                        self.builder
                            .build_store(
                                tag_ptr,
                                self.context.i32_type().const_int(Tag::Unit as u64, false),
                            )
                            .unwrap();
                        let data_ptr = self
                            .builder
                            .build_struct_gep(
                                self.runtime_value_type,
                                dummy_alloca,
                                1,
                                "dummy_data_ptr",
                            )
                            .unwrap();
                        self.builder
                            .build_store(data_ptr, self.context.i64_type().const_int(0, false))
                            .unwrap();

                        let val = self
                            .builder
                            .build_load(self.runtime_value_type, dummy_alloca, "dummy_val")
                            .unwrap();
                        self.builder.build_return(Some(&val)).unwrap();
                    }
                }
                ast::Stmt::If {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    let parent_fn = self
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_parent()
                        .unwrap();

                    let then_bb = self.context.append_basic_block(parent_fn, "then_bb");
                    let else_bb = self.context.append_basic_block(parent_fn, "else_bb");
                    let merge_bb = self.context.append_basic_block(parent_fn, "if_merge");

                    let cond_val = self.compile_expr(cond, module)?;
                    let cond_ptr = cond_val.into_pointer_value();
                    let cond_data_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, cond_ptr, 1, "cond_data_ptr")
                        .unwrap();
                    let cond_loaded = self
                        .builder
                        .build_load(self.context.i64_type(), cond_data_ptr, "cond_loaded")
                        .unwrap()
                        .into_int_value();
                    let zero = self.context.i64_type().const_int(0, false);
                    let cond_bool = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::NE,
                            cond_loaded,
                            zero,
                            "if_cond_bool",
                        )
                        .unwrap();

                    let _ = self
                        .builder
                        .build_conditional_branch(cond_bool, then_bb, else_bb);

                    self.builder.position_at_end(then_bb);
                    self.compile_block(then_blk, module)?;
                    if self
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_terminator()
                        .is_none()
                    {
                        let _ = self.builder.build_unconditional_branch(merge_bb);
                    }

                    self.builder.position_at_end(else_bb);
                    if let Some(else_blk) = else_blk {
                        self.compile_block(else_blk, module)?;
                    }
                    if self
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_terminator()
                        .is_none()
                    {
                        let _ = self.builder.build_unconditional_branch(merge_bb);
                    }

                    self.builder.position_at_end(merge_bb);
                }
                ast::Stmt::While { cond, body } => {
                    let parent_fn = self
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_parent()
                        .unwrap();

                    let cond_bb = self.context.append_basic_block(parent_fn, "while_cond");
                    let body_bb = self.context.append_basic_block(parent_fn, "while_body");
                    let after_bb = self.context.append_basic_block(parent_fn, "while_after");

                    let _ = self.builder.build_unconditional_branch(cond_bb);

                    self.builder.position_at_end(cond_bb);
                    let cond_val = self.compile_expr(cond, module)?;
                    let cond_ptr = cond_val.into_pointer_value();

                    let cond_data_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, cond_ptr, 1, "cond_data_ptr")
                        .unwrap();
                    let cond_loaded = self
                        .builder
                        .build_load(self.context.i64_type(), cond_data_ptr, "cond_loaded")
                        .unwrap()
                        .into_int_value();

                    let zero = self.context.i64_type().const_int(0, false);
                    let cond_bool = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::NE,
                            cond_loaded,
                            zero,
                            "while_cond_bool",
                        )
                        .unwrap();

                    let _ = self
                        .builder
                        .build_conditional_branch(cond_bool, body_bb, after_bb);

                    self.builder.position_at_end(body_bb);
                    self.compile_block(body, module)?;

                    if self
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_terminator()
                        .is_none()
                    {
                        let _ = self.builder.build_unconditional_branch(cond_bb);
                    }

                    self.builder.position_at_end(after_bb);
                }
                ast::Stmt::Expr(expr) => {
                    self.compile_expr(expr, module)?;
                }
            }
        }
        Ok(())
    }

    fn compile_expr(
        &mut self,
        expr: &ast::Expr,
        module: &Module<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match expr {
            ast::Expr::Number(n) => {
                let ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "num_alloc")
                    .unwrap();

                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, ptr, 0, "tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Integer as u64, false),
                    )
                    .unwrap();

                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, ptr, 1, "data_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        data_ptr,
                        self.context.i64_type().const_int(*n as u64, false),
                    )
                    .unwrap();

                Ok(ptr.into())
            }
            ast::Expr::Str(str) => {
                let global = if let Some(existing) = self.string_constants.get(str) {
                    *existing
                } else {
                    let str_val = self.context.const_string(str.as_bytes(), true);
                    let global = module.add_global(
                        str_val.get_type(),
                        Some(AddressSpace::default()),
                        &format!("str_const_{}", self.string_constants.len()),
                    );
                    global.set_initializer(&str_val);
                    global.set_linkage(Linkage::Internal);
                    global.set_constant(true);
                    self.string_constants.insert(str.clone(), global);
                    global
                };

                let ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "str_alloc")
                    .unwrap();

                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, ptr, 0, "tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(tag_ptr, self.context.i32_type().const_int(1, false))
                    .unwrap();

                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, ptr, 1, "data_ptr")
                    .unwrap();
                let str_ptr = global.as_pointer_value();
                let str_ptr_as_i64 = self
                    .builder
                    .build_ptr_to_int(str_ptr, self.context.i64_type(), "str_ptr_as_i64")
                    .unwrap();
                self.builder.build_store(data_ptr, str_ptr_as_i64).unwrap();

                Ok(ptr.into())
            }
            ast::Expr::Bool(boolean) => {
                let ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "bool_alloc")
                    .unwrap();

                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, ptr, 0, "tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, ptr, 1, "data_ptr")
                    .unwrap();
                let bool_val = if *boolean { 1 } else { 0 };
                self.builder
                    .build_store(data_ptr, self.context.i64_type().const_int(bool_val, false))
                    .unwrap();

                Ok(ptr.into())
            }
            ast::Expr::Var(ident) => {
                if let Some(var_addr) = self.variables.get(ident) {
                    Ok(*var_addr)
                } else {
                    Err(format!("Undefined variable: {}", ident))
                }
            }
            ast::Expr::Call(ident, args, _) => {
                if ident == "println" {
                    let print_fn = self.get_runtime_fn(module, "__println");

                    let list_ptr = self.build_list_from_exprs(args, module)?;

                    self.builder
                        .build_call(print_fn, &[list_ptr.into()], "println_call")
                        .unwrap();

                    let res_ptr = self
                        .builder
                        .build_alloca(self.runtime_value_type, "unit_res")
                        .unwrap();
                    let res_tag_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                        .unwrap();
                    self.builder
                        .build_store(
                            res_tag_ptr,
                            self.context.i32_type().const_int(Tag::Unit as u64, false),
                        )
                        .unwrap();

                    return Ok(res_ptr.into());
                }

                if ident == "list_push" {
                    if args.len() != 2 {
                        return Err("list_push expects 2 arguments".to_string());
                    }
                    let list_ptr = self.compile_expr(&args[0], module)?.into_pointer_value();
                    let val_ptr = self.compile_expr(&args[1], module)?.into_pointer_value();

                    let list_data_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, list_ptr, 1, "list_data_ptr")
                        .unwrap();
                    let list_vec_int = self
                        .builder
                        .build_load(self.context.i64_type(), list_data_ptr, "list_vec_int")
                        .unwrap()
                        .into_int_value();
                    let list_vec_ptr = self
                        .builder
                        .build_int_to_ptr(
                            list_vec_int,
                            self.context.ptr_type(AddressSpace::default()),
                            "list_vec_ptr",
                        )
                        .unwrap();

                    let target_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, val_ptr, 0, "val_tag_ptr")
                        .unwrap();
                    let val_tag = self
                        .builder
                        .build_load(self.context.i32_type(), target_ptr, "val_tag")
                        .unwrap()
                        .into_int_value();

                    let data_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, val_ptr, 1, "val_data_ptr")
                        .unwrap();
                    let val_data = self
                        .builder
                        .build_load(self.context.i64_type(), data_ptr, "val_data")
                        .unwrap()
                        .into_int_value();

                    let list_push_fn = self.get_runtime_fn(module, "__list_push");

                    self.builder
                        .build_call(
                            list_push_fn,
                            &[list_vec_ptr.into(), val_tag.into(), val_data.into()],
                            "list_push_call",
                        )
                        .unwrap();

                    let res_ptr = self
                        .builder
                        .build_alloca(self.runtime_value_type, "unit_res")
                        .unwrap();
                    let res_tag_ptr = self
                        .builder
                        .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                        .unwrap();
                    self.builder
                        .build_store(
                            res_tag_ptr,
                            self.context.i32_type().const_int(Tag::Unit as u64, false),
                        )
                        .unwrap();

                    return Ok(res_ptr.into());
                }

                let func = module
                    .get_function(ident)
                    .or_else(|| self.modules.values().find_map(|m| m.get_function(ident)))
                    .ok_or(format!("Undefined function: {}", ident))?;

                let mut compiled_args = Vec::with_capacity(args.len());
                for arg in args {
                    compiled_args.push(self.compile_expr(arg, module)?.into());
                }
                let call_site = self
                    .builder
                    .build_call(func, &compiled_args, "call_tmp")
                    .unwrap();

                let result_val = match call_site.try_as_basic_value() {
                    ValueKind::Basic(val) => Ok(val),
                    ValueKind::Instruction(_) => {
                        Err("Expected basic value from function call".to_string())
                    }
                };
                let result_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "call_res_alloc")
                    .unwrap();
                self.builder.build_store(result_ptr, result_val?).unwrap();
                Ok(result_ptr.into())
            }
            ast::Expr::Add(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 0, "l_tag_ptr")
                    .unwrap();
                let l_tag = self
                    .builder
                    .build_load(self.context.i32_type(), l_tag_ptr, "l_tag")
                    .unwrap()
                    .into_int_value();

                let r_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 0, "r_tag_ptr")
                    .unwrap();
                let r_tag = self
                    .builder
                    .build_load(self.context.i32_type(), r_tag_ptr, "r_tag")
                    .unwrap()
                    .into_int_value();

                // check if both are integers
                let int_tag = self
                    .context
                    .i32_type()
                    .const_int(Tag::Integer as u64, false);
                let is_l_int = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, l_tag, int_tag, "is_l_int")
                    .unwrap();
                let is_r_int = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, r_tag, int_tag, "is_r_int")
                    .unwrap();
                let both_int = self
                    .builder
                    .build_and(is_l_int, is_r_int, "both_int")
                    .unwrap();

                // check if both are strings
                let string_tag = self.context.i32_type().const_int(Tag::String as u64, false);
                let is_l_string = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, l_tag, string_tag, "is_l_string")
                    .unwrap();
                let is_r_string = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, r_tag, string_tag, "is_r_string")
                    .unwrap();

                // currently only handling int + int and string + string, for now didn't use a both_string variable
                let _both_string = self
                    .builder
                    .build_and(is_l_string, is_r_string, "both_string")
                    .unwrap();

                // create branches
                let parent_fn = self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();
                let int_bb = self.context.append_basic_block(parent_fn, "add_int_bb");
                let string_bb = self.context.append_basic_block(parent_fn, "add_string_bb");
                let merge_bb = self.context.append_basic_block(parent_fn, "add_merge_bb");

                let _ = self
                    .builder
                    .build_conditional_branch(both_int, int_bb, string_bb);

                self.builder.position_at_end(int_bb);

                let l_int_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_int_data_ptr")
                    .unwrap();
                let l_int_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_int_data_ptr, "l_int_val")
                    .unwrap()
                    .into_int_value();

                let r_int_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_int_data_ptr")
                    .unwrap();
                let r_int_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_int_data_ptr, "r_int_val")
                    .unwrap()
                    .into_int_value();

                let int_sum = self
                    .builder
                    .build_int_add(l_int_val, r_int_val, "int_sum")
                    .unwrap();

                let int_res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "int_res_alloc")
                    .unwrap();
                let int_res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, int_res_ptr, 0, "int_res_tag_ptr")
                    .unwrap();
                self.builder.build_store(int_res_tag_ptr, int_tag).unwrap();
                let int_res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, int_res_ptr, 1, "int_res_data_ptr")
                    .unwrap();
                self.builder.build_store(int_res_data_ptr, int_sum).unwrap();

                let _ = self.builder.build_unconditional_branch(merge_bb);

                self.builder.position_at_end(string_bb);

                let l_str_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_str_data_ptr")
                    .unwrap();
                let l_str_ptr_int = self
                    .builder
                    .build_load(self.context.i64_type(), l_str_data_ptr, "l_str_ptr_int")
                    .unwrap()
                    .into_int_value();
                let l_str_ptr = self
                    .builder
                    .build_int_to_ptr(
                        l_str_ptr_int,
                        self.context.ptr_type(AddressSpace::default()),
                        "l_str_ptr",
                    )
                    .unwrap();
                let r_str_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_str_data_ptr")
                    .unwrap();
                let r_str_ptr_int = self
                    .builder
                    .build_load(self.context.i64_type(), r_str_data_ptr, "r_str_ptr_int")
                    .unwrap()
                    .into_int_value();
                let r_str_ptr = self
                    .builder
                    .build_int_to_ptr(
                        r_str_ptr_int,
                        self.context.ptr_type(AddressSpace::default()),
                        "r_str_ptr",
                    )
                    .unwrap();

                let strlen_fn = self.get_runtime_fn(module, "__strlen");
                let malloc_fn = self.get_runtime_fn(module, "__malloc");

                let l_len = self
                    .builder
                    .build_call(strlen_fn, &[l_str_ptr.into()], "l_strlen_call")
                    .unwrap();

                let l_len_val = match l_len.try_as_basic_value() {
                    ValueKind::Basic(val) => val.into_int_value(),
                    _ => return Err("Expected basic value from strlen".to_string()),
                };

                let r_len = self
                    .builder
                    .build_call(strlen_fn, &[r_str_ptr.into()], "r_strlen_call")
                    .unwrap();

                let r_len_val = match r_len.try_as_basic_value() {
                    ValueKind::Basic(val) => val.into_int_value(),
                    _ => return Err("Expected basic value from strlen".to_string()),
                };

                let total_len = self
                    .builder
                    .build_int_add(l_len_val, r_len_val, "total_str_len")
                    .unwrap();
                let one = self.context.i64_type().const_int(1, false); // for null terminator
                let alloc_size = self
                    .builder
                    .build_int_add(total_len, one, "alloc_size")
                    .unwrap();

                let malloc_call = self
                    .builder
                    .build_call(malloc_fn, &[alloc_size.into()], "malloc_call")
                    .unwrap();

                let malloc_ptr = match malloc_call.try_as_basic_value() {
                    ValueKind::Basic(val) => val.into_pointer_value(),
                    _ => return Err("Expected basic value from malloc".to_string()),
                };

                self.builder
                    .build_memcpy(malloc_ptr, 1, l_str_ptr, 1, l_len_val)
                    .unwrap();

                let dest_ptr = unsafe {
                    self.builder
                        .build_gep(self.context.i8_type(), malloc_ptr, &[l_len_val], "dest_ptr")
                        .unwrap()
                };
                self.builder
                    .build_memcpy(dest_ptr, 1, r_str_ptr, 1, r_len_val)
                    .unwrap();

                let end_ptr = unsafe {
                    self.builder
                        .build_gep(self.context.i8_type(), malloc_ptr, &[total_len], "end_ptr")
                        .unwrap()
                };
                self.builder
                    .build_store(end_ptr, self.context.i8_type().const_int(0, false))
                    .unwrap();

                let str_res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "str_res_alloc")
                    .unwrap();

                let str_res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, str_res_ptr, 0, "str_res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(str_res_tag_ptr, string_tag)
                    .unwrap();

                let str_res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, str_res_ptr, 1, "str_res_data_ptr")
                    .unwrap();
                let malloc_ptr_as_i64 = self
                    .builder
                    .build_ptr_to_int(malloc_ptr, self.context.i64_type(), "malloc_ptr_as_i64")
                    .unwrap();
                self.builder
                    .build_store(str_res_data_ptr, malloc_ptr_as_i64)
                    .unwrap();

                let _ = self.builder.build_unconditional_branch(merge_bb);

                self.builder.position_at_end(merge_bb);
                let phi = self
                    .builder
                    .build_phi(
                        self.context.ptr_type(AddressSpace::default()),
                        "add_res_phi",
                    )
                    .unwrap();
                phi.add_incoming(&[(&int_res_ptr, int_bb), (&str_res_ptr, string_bb)]);

                Ok(phi.as_basic_value())
            }
            ast::Expr::Mul(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let product = self.builder.build_int_mul(l_val, r_val, "product").unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Integer as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                self.builder.build_store(res_data_ptr, product).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Minus(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let difference = self
                    .builder
                    .build_int_sub(l_val, r_val, "difference")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Integer as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                self.builder.build_store(res_data_ptr, difference).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Div(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let quotient = self
                    .builder
                    .build_int_signed_div(l_val, r_val, "quotient")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Integer as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                self.builder.build_store(res_data_ptr, quotient).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Mod(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let remainder = self
                    .builder
                    .build_int_signed_rem(l_val, r_val, "remainder")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Integer as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                self.builder.build_store(res_data_ptr, remainder).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Increment(expr) => {
                let val_ptr = self.compile_expr(expr, module)?.into_pointer_value();

                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, val_ptr, 1, "data_ptr")
                    .unwrap();
                let val = self
                    .builder
                    .build_load(self.context.i64_type(), data_ptr, "val")
                    .unwrap()
                    .into_int_value();

                let one = self.context.i64_type().const_int(1, false);
                let incremented = self.builder.build_int_add(val, one, "incremented").unwrap();

                self.builder.build_store(data_ptr, incremented).unwrap();

                Ok(val_ptr.into())
            }
            ast::Expr::Decrement(expr) => {
                let val_ptr = self.compile_expr(expr, module)?.into_pointer_value();

                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, val_ptr, 1, "data_ptr")
                    .unwrap();
                let val = self
                    .builder
                    .build_load(self.context.i64_type(), data_ptr, "val")
                    .unwrap()
                    .into_int_value();

                let one = self.context.i64_type().const_int(1, false);
                let decremented = self.builder.build_int_sub(val, one, "decremented").unwrap();

                self.builder.build_store(data_ptr, decremented).unwrap();

                Ok(val_ptr.into())
            }
            ast::Expr::Eq(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let eq = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, l_val, r_val, "eq")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let bool_as_i64 = self
                    .builder
                    .build_int_z_extend(eq, self.context.i64_type(), "bool_as_i64")
                    .unwrap();
                self.builder.build_store(res_data_ptr, bool_as_i64).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Neq(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let neq = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::NE, l_val, r_val, "neq")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let bool_as_i64 = self
                    .builder
                    .build_int_z_extend(neq, self.context.i64_type(), "bool_as_i64")
                    .unwrap();
                self.builder.build_store(res_data_ptr, bool_as_i64).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Gt(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let gt = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::SGT, l_val, r_val, "gt")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let bool_as_i64 = self
                    .builder
                    .build_int_z_extend(gt, self.context.i64_type(), "bool_as_i64")
                    .unwrap();
                self.builder.build_store(res_data_ptr, bool_as_i64).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Lt(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let lt = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::SLT, l_val, r_val, "lt")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let bool_as_i64 = self
                    .builder
                    .build_int_z_extend(lt, self.context.i64_type(), "bool_as_i64")
                    .unwrap();
                self.builder.build_store(res_data_ptr, bool_as_i64).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Ge(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let ge = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::SGE, l_val, r_val, "ge")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let bool_as_i64 = self
                    .builder
                    .build_int_z_extend(ge, self.context.i64_type(), "bool_as_i64")
                    .unwrap();
                self.builder.build_store(res_data_ptr, bool_as_i64).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Le(lhs, rhs) => {
                let l_ptr = self.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self.compile_expr(rhs, module)?.into_pointer_value();

                let l_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, l_ptr, 1, "l_data_ptr")
                    .unwrap();
                let l_val = self
                    .builder
                    .build_load(self.context.i64_type(), l_data_ptr, "l_val")
                    .unwrap()
                    .into_int_value();

                let r_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, r_ptr, 1, "r_data_ptr")
                    .unwrap();
                let r_val = self
                    .builder
                    .build_load(self.context.i64_type(), r_data_ptr, "r_val")
                    .unwrap()
                    .into_int_value();

                let le = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::SLE, l_val, r_val, "le")
                    .unwrap();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let bool_as_i64 = self
                    .builder
                    .build_int_z_extend(le, self.context.i64_type(), "bool_as_i64")
                    .unwrap();
                self.builder.build_store(res_data_ptr, bool_as_i64).unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::If(cond, then_expr, else_expr) => {
                let parent_fn = self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();

                let then_bb = self.context.append_basic_block(parent_fn, "then_bb");
                let else_bb = self.context.append_basic_block(parent_fn, "else_bb");
                let merge_bb = self.context.append_basic_block(parent_fn, "if_merge");

                let cond_val = self.compile_expr(cond, module)?;
                let cond_ptr = cond_val.into_pointer_value();
                let cond_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, cond_ptr, 1, "cond_data_ptr")
                    .unwrap();
                let cond_loaded = self
                    .builder
                    .build_load(self.context.i64_type(), cond_data_ptr, "cond_loaded")
                    .unwrap()
                    .into_int_value();
                let zero = self.context.i64_type().const_int(0, false);
                let cond_bool = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::NE, cond_loaded, zero, "if_cond_bool")
                    .unwrap();

                let _ = self
                    .builder
                    .build_conditional_branch(cond_bool, then_bb, else_bb);

                self.builder.position_at_end(then_bb);
                let then_val = self.compile_expr(then_expr, module)?;
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    let _ = self.builder.build_unconditional_branch(merge_bb);
                }
                let then_bb_end = self.builder.get_insert_block().unwrap();

                // TODO: Handle case where else_expr, such as if (test) : ok() ? no();
                // TODO: Also  such as if (test) ok() orelse no();

                self.builder.position_at_end(else_bb);
                let else_val = self.compile_expr(else_expr, module)?;
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    let _ = self.builder.build_unconditional_branch(merge_bb);
                }
                let else_bb_end = self.builder.get_insert_block().unwrap();

                self.builder.position_at_end(merge_bb);
                let phi = self
                    .builder
                    .build_phi(self.runtime_value_type, "if_phi")
                    .unwrap();

                if then_bb_end
                    .get_terminator()
                    .map_or(false, |t| t.get_parent().unwrap() == merge_bb)
                {
                    phi.add_incoming(&[(&then_val, then_bb_end)]);
                }
                if else_bb_end
                    .get_terminator()
                    .map_or(false, |t| t.get_parent().unwrap() == merge_bb)
                {
                    phi.add_incoming(&[(&else_val, else_bb_end)]);
                }

                Ok(phi.as_basic_value())
            }
            ast::Expr::List(elements) => {
                let list_ptr = self.build_list_from_exprs(elements, module)?;
                let i64_type = self.context.i64_type();

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "list_res_alloc")
                    .unwrap();
                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context.i32_type().const_int(Tag::List as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let list_ptr_as_int = self
                    .builder
                    .build_ptr_to_int(list_ptr, i64_type, "list_ptr_as_int")
                    .unwrap();
                self.builder
                    .build_store(res_data_ptr, list_ptr_as_int)
                    .unwrap();

                Ok(res_ptr.into())
            }
            ast::Expr::Index(collection_expr, index_expr) => {
                let get_fn = self.get_runtime_fn(module, "__list_get");

                let collection_var_ptr = self
                    .compile_expr(collection_expr, module)?
                    .into_pointer_value();

                let list_data_ptr = self
                    .builder
                    .build_struct_gep(
                        self.runtime_value_type,
                        collection_var_ptr,
                        1,
                        "list_data_ptr",
                    )
                    .unwrap();
                let list_ptr_int = self
                    .builder
                    .build_load(self.context.i64_type(), list_data_ptr, "list_ptr_int")
                    .unwrap()
                    .into_int_value();

                let list_ptr = self
                    .builder
                    .build_int_to_ptr(
                        list_ptr_int,
                        self.context.ptr_type(AddressSpace::default()),
                        "list_ptr",
                    )
                    .unwrap();

                let index_val_ptr = self.compile_expr(index_expr, module)?.into_pointer_value();

                let index_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, index_val_ptr, 1, "index_data_ptr")
                    .unwrap();
                let index_int = self
                    .builder
                    .build_load(self.context.i64_type(), index_data_ptr, "index_int")
                    .unwrap()
                    .into_int_value();

                let get_call = self
                    .builder
                    .build_call(
                        get_fn,
                        &[list_ptr.into(), index_int.into()],
                        "list_get_call",
                    )
                    .unwrap();

                match get_call.try_as_basic_value() {
                    ValueKind::Basic(val) => Ok(val),
                    ValueKind::Instruction(_) => {
                        Err("Expected basic value from __list_get".to_string())
                    }
                }
            }
            ast::Expr::Range(start_expr, end_expr) => {
                let range_fn = self.get_runtime_fn(module, "__range_new");
                let start_val_ptr = self.compile_expr(start_expr, module)?.into_pointer_value();
                let start_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, start_val_ptr, 1, "start_data_ptr")
                    .unwrap();
                let start_int = self
                    .builder
                    .build_load(self.context.i64_type(), start_data_ptr, "start_int")
                    .unwrap()
                    .into_int_value();

                let end_val_ptr = self.compile_expr(end_expr, module)?.into_pointer_value();
                let end_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, end_val_ptr, 1, "end_data_ptr")
                    .unwrap();
                let end_int = self
                    .builder
                    .build_load(self.context.i64_type(), end_data_ptr, "end_int")
                    .unwrap()
                    .into_int_value();

                let range_call = self
                    .builder
                    .build_call(range_fn, &[start_int.into(), end_int.into()], "range_call")
                    .unwrap();
                let range_ptr = match range_call.try_as_basic_value() {
                    ValueKind::Basic(val) => val.into_pointer_value(),
                    ValueKind::Instruction(_) => {
                        return Err("Expected basic value from __range_new".to_string());
                    }
                };

                let res_ptr = self
                    .builder
                    .build_alloca(self.runtime_value_type, "range_res_alloc")
                    .unwrap();

                let res_tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self.builder
                    .build_store(
                        res_tag_ptr,
                        self.context.i32_type().const_int(Tag::Range as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let range_ptr_as_int = self
                    .builder
                    .build_ptr_to_int(range_ptr, self.context.i64_type(), "range_ptr_as_int")
                    .unwrap();
                self.builder
                    .build_store(res_data_ptr, range_ptr_as_int)
                    .unwrap();
                Ok(res_ptr.into())
            }
            ast::Expr::ModuleAccess(module_name, function_name, args) => {
                // !TODO Implement module access
                Err("Module access not implemented".to_string())
            }
            _ => Err("Not implemented".to_string()),
        }
    }
}

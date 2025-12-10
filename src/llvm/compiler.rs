use crate::command_helper;
use crate::front::ast;
use crate::interpreter::runner::parse_only;
use crate::interpreter::type_helper;
use crate::interpreter::type_helper::Type;
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
use std::f32::consts::E;
use std::result;

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub modules: HashMap<String, Module<'ctx>>, // name, module
    pub builder: Builder<'ctx>,
    pub scopes: Vec<Scope<'ctx>>,
    pub function_signatures: Option<FunctionValue<'ctx>>,
    pub runtime_value_type: StructType<'ctx>,
    pub target_os: OS,
    pub string_constants: HashMap<String, inkwell::values::GlobalValue<'ctx>>,
    pub malloc_type: inkwell::types::FunctionType<'ctx>,
    pub source_path: String,
    pub struct_defs: HashMap<String, HashMap<String, u32>>, // struct name -> (field name -> field type)
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum OS {
    Unknown, // default triple
    Windows,
    Linux,
}

pub enum Tag {
    // Dynamic value tags
    Integer = 0, // i64
    Float = 1,   // f64
    String = 2,
    Boolean = 3,
    List = 4,
    Range = 5,
    Unit = 6,
    Enum = 7,

    // System types
    Int8 = 100,
    Uint8 = 101,
    Int16 = 102,
    Uint16 = 103,
    Int32 = 104,
    Uint32 = 105,
    Int64 = 106,
    Uint64 = 107,

    Float16 = 108,
    Float32 = 109,
    Float64 = 110,
}

const WINDOWS_STR: &str = "Windows";
const LINUX_STR: &str = "Linux";

pub struct Scope<'ctx> {
    pub variables: HashMap<String, (BasicValueEnum<'ctx>, Type)>,
    pub var_name: Vec<String>,
}

impl<'ctx> Scope<'ctx> {
    pub fn new() -> Self {
        Scope {
            variables: HashMap::new(),
            var_name: Vec::new(),
        }
    }
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, builder: Builder<'ctx>, source_path: String) -> Self {
        let runtime_value_type = context.struct_type(
            &[context.i32_type().into(), context.i64_type().into()],
            false,
        );

        let i64_type = context.i64_type();
        let i8_ptr_type = context.ptr_type(AddressSpace::default());
        let malloc_type = i8_ptr_type.fn_type(&[i64_type.into()], false);

        // scope index 0 equals global scope
        let mut scopes = Vec::new();
        scopes.push(Scope::new());

        Compiler {
            context,
            modules: HashMap::new(),
            builder,
            scopes,
            function_signatures: None,
            runtime_value_type,
            target_os: OS::Unknown,
            string_constants: HashMap::new(),
            malloc_type,
            source_path,
            struct_defs: HashMap::new(),
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn exit_scope(&mut self, module: &Module<'ctx>) {
        let scope = self.scopes.pop().unwrap();

        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            let drop_fn = self.get_runtime_fn(module, "__drop");

            for name in scope.var_name.iter().rev() {
                if let Some((val, _)) = scope.variables.get(name) {
                    if val.is_pointer_value() {
                        builder_helper::drop_var(self, val.into_pointer_value(), drop_fn, name);
                    }
                }
            }
        }
    }

    pub fn get_variables(&self, name: &str) -> Option<(BasicValueEnum<'ctx>, Type)> {
        for scope in self.scopes.iter().rev() {
            if let Some(var) = scope.variables.get(name) {
                return Some(var.clone());
            }
        }
        None
    }

    pub fn add_variable(&mut self, name: String, value: BasicValueEnum<'ctx>, ty: Type) {
        if let Some(current_scope) = self.scopes.last_mut() {
            current_scope.variables.insert(name.clone(), (value, ty));
            current_scope.var_name.push(name);
        }
    }

    pub fn remove_variable(&mut self, name: &str) {
        if let Some(current_scope) = self.scopes.last_mut() {
            current_scope.variables.remove(name);
        }
    }

    fn emit_drop_for_return(&mut self, module: &Module<'ctx>) {
        let drop_fn = self.get_runtime_fn(module, "__drop");

        let mut vars_to_drop: Vec<(PointerValue<'ctx>, String)> = Vec::new();

        for scope in self.scopes.iter().skip(1).rev() {
            for name in scope.var_name.iter().rev() {
                if let Some((val, _)) = scope.variables.get(name) {
                    if val.is_pointer_value() {
                        vars_to_drop.push((val.into_pointer_value(), name.clone()));
                    }
                }
            }
        }

        for (ptr, var_name) in vars_to_drop.into_iter().rev() {
            builder_helper::drop_var(self, ptr, drop_fn, &var_name);
        }
    }

    pub fn register_struct(&mut self, name: String, fields: Vec<String>) {
        let mut field_map = HashMap::new();
        for (i, field) in fields.iter().enumerate() {
            field_map.insert(field.clone(), i as u32);
        }
        self.struct_defs.insert(name, field_map);
    }

    pub fn get_field_index(&self, struct_name: &str, field_name: &str) -> Result<u32, String> {
        self.struct_defs
            .get(struct_name)
            .and_then(|fields| fields.get(field_name).cloned())
            .ok_or_else(|| {
                format!(
                    "Field '{}' not found in struct '{}'",
                    field_name, struct_name
                )
            })
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
            "__panic" => void_type.fn_type(&[i8_ptr_type.into()], false),
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

        let items = parse_only(&source, &path)?;

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

        // First, load and compile all imports
        for item in &items {
            if let ast::Item::Import(import_name) = item {
                self.load_and_compile_module(import_name, None)?;
            }
        }
        // Declare all function prototypes
        for item in &items {
            match item {
                ast::Item::FunctionItem(func) => {
                    self.declare_fn_prototype(func, &module);
                }
                _ => {}
            }
        }

        let mut private_enum_variants: Vec<String> = Vec::new();

        // get enums
        for item in &items {
            match item {
                ast::Item::EnumItem(enm) => {
                    self.register_enum(enm);

                    if !enm.is_public {
                        for variant in &enm.variants {
                            let full_name = format!("{}.{}", enm.ident, variant);
                            private_enum_variants.push(full_name);
                        }
                    }
                }
                _ => {}
            }
        }

        // Now compile all functions
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

        for private_variant in private_enum_variants {
            self.remove_variable(&private_variant);
        }

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

    fn register_enum(&mut self, enm: &ast::Enum) {
        if enm.variants.is_empty() {
            return;
        }

        for (idx, variant) in enm.variants.iter().enumerate() {
            let full_name = format!("{}.{}", enm.ident, variant);
            let tag_value = idx as i32;

            self.add_variable(
                full_name,
                BasicValueEnum::IntValue(
                    self.context.i32_type().const_int(tag_value as u64, false),
                ),
                Type::Enum,
            );
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

        let fn_type = if let Some(ret_ty) = &func.ret_ty {
            match ret_ty {
                Type::Any => self.runtime_value_type.fn_type(&arg_types, false),
                Type::Int => self.context.i64_type().fn_type(&arg_types, false),
                Type::Str => self
                    .context
                    .ptr_type(AddressSpace::default())
                    .fn_type(&arg_types, false),
                Type::Float => self.context.f64_type().fn_type(&arg_types, false),
                Type::Bool => self.context.bool_type().fn_type(&arg_types, false),
                Type::Unit => self.context.void_type().fn_type(&arg_types, false),
                Type::Enum => self.context.i64_type().fn_type(&arg_types, false),
                Type::Struct(_) => self.runtime_value_type.fn_type(&arg_types, false),

                Type::TypeI8 => self.context.i8_type().fn_type(&arg_types, false),
                Type::TypeU8 => self.context.i8_type().fn_type(&arg_types, false),
                Type::TypeI16 => self.context.i16_type().fn_type(&arg_types, false),
                Type::TypeU16 => self.context.i16_type().fn_type(&arg_types, false),
                Type::TypeI32 => self.context.i32_type().fn_type(&arg_types, false),
                Type::TypeU32 => self.context.i32_type().fn_type(&arg_types, false),
                Type::TypeI64 => self.context.i64_type().fn_type(&arg_types, false),
                Type::TypeU64 => self.context.i64_type().fn_type(&arg_types, false),

                Type::TypeF16 => self.context.f16_type().fn_type(&arg_types, false),
                Type::TypeF32 => self.context.f32_type().fn_type(&arg_types, false),
                Type::TypeF64 => self.context.f64_type().fn_type(&arg_types, false),
            }
        } else {
            self.runtime_value_type.fn_type(&arg_types, false)
        };

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

        if !func.is_public {
            fn_val.set_linkage(Linkage::Private);
        }
    }

    pub fn get_known_type_from_expr(&self, expr: &ast::Expr) -> Result<String, String> {
        match expr {
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

    fn infer_type(&self, expr: &ast::Expr) -> Type {
        match expr {
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
            ast::Expr::Increment(value) | ast::Expr::Decrement(value) => self.infer_type(value),
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
            "_sprs_main"
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
            self.exit_scope(module);
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

    pub(crate) fn compile_block(
        &mut self,
        stmts: &Vec<ast::Stmt>,
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

            match stmt {
                ast::Stmt::Var(var) => {
                    let init_val = self
                        .compile_expr(&var.expr.as_ref().unwrap_or(&ast::Expr::Unit()), module)?
                        .into_pointer_value();

                    let var_type =
                        self.infer_type(&var.expr.as_ref().unwrap_or(&ast::Expr::Unit()));

                    builder_helper::var_load_at_init_variable(self, init_val, &var.ident);

                    if let Some(ast::Expr::Var(src_val_name)) = &var.expr {
                        let var_val = self.get_variables(src_val_name).map(|(v, _)| v);
                        if let Some(val) = var_val {
                            builder_helper::move_variable(self, &val, &var.ident);
                        }
                    }
                    self.add_variable(var.ident.clone(), init_val.into(), var_type);
                }
                ast::Stmt::Return(expr_opt) => {
                    let ret_val = if let Some(expr) = expr_opt {
                        let ptr = self.compile_expr(expr, module)?.into_pointer_value();

                        if let ast::Expr::Var(name) = expr {
                            let var_val = self.get_variables(name).map(|(v, _)| v);
                            if let Some(val) = var_val {
                                let val_ptr = val.into_pointer_value();
                                builder_helper::var_return_store(self, &val_ptr.into(), name);
                            }
                        }

                        let current_fn = self.function_signatures.unwrap();
                        let return_type = current_fn.get_type().get_return_type();

                        let expr_type = self.infer_type(expr);

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

                        if let Some(ret_ty) = return_type {
                            if ret_ty == self.runtime_value_type.into() {
                                let val = self
                                    .builder
                                    .build_load(self.runtime_value_type, ptr, "return_load")
                                    .unwrap();
                                Some(val)
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
                                        .build_bit_cast(
                                            data_val,
                                            self.context.f64_type(),
                                            "casted_float",
                                        )
                                        .unwrap()
                                        .into_float_value();

                                    if float_type.get_bit_width() == 32 {
                                        self.builder
                                            .build_float_trunc(
                                                f64_val,
                                                float_type,
                                                "truncated_float",
                                            )
                                            .unwrap()
                                            .into()
                                    } else {
                                        f64_val.into()
                                    }
                                } else if ret_ty.is_pointer_type() {
                                    let ptr_type = ret_ty.into_pointer_type();
                                    let i8_ptr = self
                                        .builder
                                        .build_int_to_ptr(data_val, ptr_type, "int_to_ptr")
                                        .unwrap()
                                        .into();
                                    i8_ptr
                                } else {
                                    return Err("Unsupported return type conversion".to_string());
                                };
                                Some(casted_val)
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    self.emit_drop_for_return(module);

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
                    self.register_enum(enm);
                }
                ast::Stmt::Assign(assign_stmt) => {
                    let val_ptr = self
                        .compile_expr(&assign_stmt.expr, module)?
                        .into_pointer_value();

                    let (target_val, _) = self
                        .get_variables(&assign_stmt.name)
                        .ok_or_else(|| format!("Undefined variable: {}", &assign_stmt.name))?;

                    let target_ptr = target_val.into_pointer_value();

                    let drop_fn = self.get_runtime_fn(module, "__drop");
                    builder_helper::drop_var(self, target_ptr, drop_fn, &assign_stmt.name);

                    let new_val = self
                        .builder
                        .build_load(self.runtime_value_type, val_ptr, "assign_load")
                        .unwrap();
                    self.builder
                        .build_store(target_ptr, new_val)
                        .map_err(|e| e.to_string())?;

                    if let ast::Expr::Var(src_val_name) = &assign_stmt.expr {
                        let var_val = self.get_variables(src_val_name).map(|(v, _)| v);
                        if let Some(val) = var_val {
                            builder_helper::move_variable(self, &val, &assign_stmt.name);
                        }
                    }
                }
            }
        }

        self.exit_scope(module);

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
            ast::Expr::Float(fp) => {
                let result = builder_helper::create_float(self, *fp);
                result
            }
            ast::Expr::TypeI8 => {
                let result = builder_helper::create_int8(self);
                result
            }
            ast::Expr::TypeU8 => {
                let result = builder_helper::create_uint8(self);
                result
            }
            ast::Expr::TypeI16 => {
                let result = builder_helper::create_int16(self);
                result
            }
            ast::Expr::TypeU16 => {
                let result = builder_helper::create_uint16(self);
                result
            }
            ast::Expr::TypeI32 => {
                let result = builder_helper::create_int32(self);
                result
            }
            ast::Expr::TypeU32 => {
                let result = builder_helper::create_uint32(self);
                result
            }
            ast::Expr::TypeI64 => {
                let result = builder_helper::create_int64(self);
                result
            }
            ast::Expr::TypeU64 => {
                let result = builder_helper::create_uint64(self);
                result
            }
            ast::Expr::TypeF16 => {
                let result = builder_helper::create_float16(self);
                result
            }
            ast::Expr::TypeF32 => {
                let result = builder_helper::create_float32(self);
                result
            }
            ast::Expr::TypeF64 => {
                let result = builder_helper::create_float64(self);
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
                if let Some((var_addr, _)) = self.get_variables(ident) {
                    Ok(var_addr)
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

                if ident == "cast!" {
                    let result = builder_helper::call_builtin_macro_cast(self, args, module);
                    return result;
                }

                let result = builder_helper::create_call_expr(self, ident, args, module);
                result
            }
            ast::Expr::FieldAccess(lhs, rhs) => {
                let lhs_type = self.infer_type(lhs);

                let struct_name = match lhs_type {
                    Type::Struct(name) => name,
                    _ => {
                        return Err(format!("Field access on non-struct type: {:?}", lhs_type));
                    }
                };

                let index = self.get_field_index(&struct_name, rhs)?;

                let result = builder_helper::create_field_access(self, lhs, index, module);
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
        }
    }
}

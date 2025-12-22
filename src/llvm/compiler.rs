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
use inkwell::types::BasicTypeEnum;
use inkwell::types::{BasicMetadataTypeEnum, StructType};
use inkwell::values::GlobalValue;
use inkwell::values::IntValue;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use std::collections::HashMap;
use std::collections::HashSet;

pub struct StructDef<'ctx> {
    pub fields: Vec<ast::StructField>,
    pub field_indices: HashMap<String, u32>,
    pub llvm_type: StructType<'ctx>,
}

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
    pub struct_defs: HashMap<String, StructDef<'ctx>>, // struct name -> struct definition
    pub enum_names: HashSet<String>,
}

pub enum StoreTag<'ctx> {
    Int(u64),
    Dynamic(IntValue<'ctx>),
}

pub enum StoreValue<'ctx> {
    Int(IntValue<'ctx>),
    Float(f64),
    Ptr(PointerValue<'ctx>),
    Bool(IntValue<'ctx>),
}

pub enum StrConstantAction {
    Get,
    Set,
}

pub enum StrConstantResult<'ctx> {
    Global(GlobalValue<'ctx>),
    Pointer(PointerValue<'ctx>),
}

pub enum StrValue<'ctx> {
    Get(&'ctx str),
    Set(GlobalValue<'ctx>),
}

// Support builder_helper.rs for LLVM instuctions of execution.
impl<'ctx> Compiler<'ctx> {
    // Default options is i64 integer store
    pub fn build_runtime_value_store(
        &self,
        target_ptr: PointerValue<'ctx>,
        tag: StoreTag<'ctx>,
        value: StoreValue<'ctx>,
        name: &str,
    ) {
        let tag_val = match tag {
            StoreTag::Int(t) => self.context.i32_type().const_int(t, false),
            StoreTag::Dynamic(t) => t,
        };

        let tag_ptr = self
            .builder
            .build_struct_gep(
                self.runtime_value_type,
                target_ptr,
                0,
                &format!("{}_tag_ptr", name),
            )
            .unwrap();
        self.builder.build_store(tag_ptr, tag_val).unwrap();

        let data_ptr = self
            .builder
            .build_struct_gep(
                self.runtime_value_type,
                target_ptr,
                1,
                &format!("{}_data_ptr", name),
            )
            .unwrap();

        let data_val = match value {
            StoreValue::Int(v) => v,
            StoreValue::Float(f) => self.context.i64_type().const_int(f.to_bits(), false),
            StoreValue::Ptr(p) => self
                .builder
                .build_ptr_to_int(p, self.context.i64_type(), "ptr_to_int")
                .unwrap(),
            StoreValue::Bool(b) => self
                .builder
                .build_int_z_extend(b, self.context.i64_type(), name)
                .unwrap(),
        };

        self.builder.build_store(data_ptr, data_val).unwrap();
    }
    pub fn tag_only_runtime_value_store(
        &self,
        target_ptr: PointerValue<'ctx>,
        tag: u64,
        name: &str,
    ) {
        let tag_val = self.context.i32_type().const_int(tag, false);

        let tag_ptr = self
            .builder
            .build_struct_gep(
                self.runtime_value_type,
                target_ptr,
                0,
                &format!("{}_tag_ptr", name),
            )
            .unwrap();
        self.builder.build_store(tag_ptr, tag_val).unwrap();
    }
    pub fn build_sprs_value_call_func(
        &self,
        ptr: PointerValue<'ctx>,
        func: FunctionValue<'_>,
        name: &str,
        extra_args: &[BasicValueEnum<'ctx>],
        is_extra_args_front_call: bool,
    ) {
        let tag_ptr = self
            .builder
            .build_struct_gep(
                self.runtime_value_type,
                ptr,
                0,
                &format!("{}_tag_ptr", name),
            )
            .unwrap();
        let tag = self
            .builder
            .build_load(self.context.i32_type(), tag_ptr, &format!("{}_tag", name))
            .unwrap()
            .into_int_value();

        let data_ptr = self
            .builder
            .build_struct_gep(
                self.runtime_value_type,
                ptr,
                1,
                &format!("{}_data_ptr", name),
            )
            .unwrap();
        let data = self
            .builder
            .build_load(self.context.i64_type(), data_ptr, &format!("{}_data", name))
            .unwrap()
            .into_int_value();

        if is_extra_args_front_call {
            let mut args = Vec::with_capacity(2 + extra_args.len());
            for extra in extra_args {
                args.push((*extra).into());
            }
            args.push(tag.into());
            args.push(data.into());
            self.builder
                .build_call(func, &args, &format!("call_{}", name))
                .unwrap();
            return;
        }

        let mut args = Vec::with_capacity(2 + extra_args.len());
        args.push(tag.into());
        args.push(data.into());
        for extra in extra_args {
            args.push((*extra).into());
        }
        self.builder
            .build_call(func, &args, &format!("call_{}", name))
            .unwrap();
    }

    pub fn set_global_constant_str(
        &mut self,
        module: &Module<'ctx>,
        str_value: StrValue<'ctx>,
        action: StrConstantAction,
        is_global: bool,
        is_const: bool,
    ) -> Option<StrConstantResult<'ctx>> {
        match action {
            StrConstantAction::Get => {
                if let Some(global) = self.string_constants.get(match str_value {
                    StrValue::Get(s) => s,
                    _ => return None,
                }) {
                    return Some(StrConstantResult::Global(*global));
                } else {
                    let global_name = if is_global {
                        format!("str_const_global_{}", self.string_constants.len())
                    } else {
                        format!("str_const_const_{}", self.string_constants.len())
                    };
                    let str_const = self.context.const_string(
                        match str_value {
                            StrValue::Get(s) => s.as_bytes(),
                            _ => return None,
                        },
                        true,
                    );
                    let global_str = module.add_global(
                        str_const.get_type(),
                        Some(AddressSpace::default()),
                        &global_name,
                    );
                    global_str.set_initializer(&str_const);
                    if is_const {
                        global_str.set_constant(true);
                    }

                    match if is_global {
                        Linkage::External
                    } else {
                        Linkage::Internal
                    } {
                        Linkage::External => global_str.set_linkage(Linkage::External),
                        Linkage::Internal => global_str.set_linkage(Linkage::Internal),
                        _ => {}
                    }

                    self.string_constants.insert(
                        match str_value {
                            StrValue::Get(s) => s.to_string(),
                            _ => "".to_string(),
                        },
                        global_str,
                    );
                    return Some(StrConstantResult::Global(global_str));
                }
            }
            StrConstantAction::Set => {
                let name_ptr = match str_value {
                    StrValue::Set(g) => g,
                    _ => return None,
                };

                let str_ptr = name_ptr.as_pointer_value();

                return Some(StrConstantResult::Pointer(str_ptr));
            }
        }
    }
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
    Struct = 8,

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
            enum_names: HashSet::new(),
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

    pub fn register_struct(&mut self, name: String, fields: Vec<ast::StructField>) {
        let mut field_indices = HashMap::new();
        let mut llvm_field_types: Vec<BasicTypeEnum> = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            field_indices.insert(field.ident.clone(), i as u32);

            let llvm_ty = if let Some(ty) = &field.ty {
                match ty {
                    Type::Any => self.runtime_value_type.into(),
                    Type::Int => self.context.i64_type().into(),
                    Type::Str => self.context.ptr_type(AddressSpace::default()).into(),
                    Type::Float => self.context.f64_type().into(),
                    Type::Bool => self.context.bool_type().into(),
                    Type::Unit => self.runtime_value_type.into(),
                    Type::Enum => self.context.i64_type().into(),
                    Type::Struct(_) => self.runtime_value_type.into(),

                    Type::TypeI8 => self.context.i8_type().into(),
                    Type::TypeU8 => self.context.i8_type().into(),
                    Type::TypeI16 => self.context.i16_type().into(),
                    Type::TypeU16 => self.context.i16_type().into(),
                    Type::TypeI32 => self.context.i32_type().into(),
                    Type::TypeU32 => self.context.i32_type().into(),
                    Type::TypeI64 => self.context.i64_type().into(),
                    Type::TypeU64 => self.context.i64_type().into(),
                    Type::TypeF16 => self.context.f16_type().into(),
                    Type::TypeF32 => self.context.f32_type().into(),
                    Type::TypeF64 => self.context.f64_type().into(),
                }
            } else {
                self.runtime_value_type.into()
            };
            llvm_field_types.push(llvm_ty);
        }

        let llvm_type = self.context.struct_type(&llvm_field_types, false);

        self.struct_defs.insert(
            name,
            StructDef {
                fields,
                field_indices,
                llvm_type,
            },
        );
    }

    pub fn get_field_index(&self, struct_name: &str, field_name: &str) -> Result<u32, String> {
        self.struct_defs
            .get(struct_name)
            .and_then(|def| def.field_indices.get(field_name).cloned())
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

        self.builder.clear_insertion_position();

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
        let mut private_struct_fields: Vec<String> = Vec::new();

        // get enums and structs first
        for item in &items {
            match item {
                ast::Item::StructItem(items) => {
                    self.register_struct(items.ident.clone(), items.fields.clone());

                    if !items.is_public {
                        for field in &items.fields {
                            let full_name = format!("{}.{}", items.ident, field.ident);
                            private_struct_fields.push(full_name);
                        }
                    }
                }
                ast::Item::EnumItem(enm) => {
                    self.register_enum(enm, &module, true);

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

        for private_field in private_struct_fields {
            self.remove_variable(&private_field);
        }

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

    fn register_enum(&mut self, enm: &ast::Enum, module: &Module<'ctx>, is_global: bool) {
        if enm.variants.is_empty() {
            return;
        }

        self.enum_names.insert(enm.ident.clone());

        // For the runtime EnumInfo struct type : { i8*, i64 }
        let i8_ptr_type = self.context.ptr_type(AddressSpace::default());
        let enum_info_type = self.context.struct_type(
            &[
                i8_ptr_type.into(),             // name
                self.context.i64_type().into(), // variant_index
            ],
            false,
        );

        for (idx, variant) in enm.variants.iter().enumerate() {
            let full_name = format!("{}.{}", enm.ident, variant);

            let enum_tag = self.context.i32_type().const_int(Tag::Enum as u64, false);

            let ptr = if !is_global {
                let current_block = self.builder.get_insert_block().unwrap();
                let function = current_block.get_parent().unwrap();
                let entry_block = function.get_first_basic_block().unwrap();

                if let Some(first_instr) = entry_block.get_first_instruction() {
                    self.builder.position_before(&first_instr)
                } else {
                    self.builder.position_at_end(entry_block)
                };

                let name_ptr = self
                    .builder
                    .build_global_string_ptr(&full_name, &format!("enum_name_{}", full_name))
                    .unwrap();

                let enum_info_ptr = self
                    .builder
                    .build_malloc(enum_info_type, &format!("enum_info_{}", full_name))
                    .unwrap();

                let name_gep = self
                    .builder
                    .build_struct_gep(enum_info_type, enum_info_ptr, 0, "name_ptr")
                    .unwrap();
                self.builder
                    .build_store(name_gep, name_ptr.as_pointer_value())
                    .unwrap();

                let idx_gep = self
                    .builder
                    .build_struct_gep(enum_info_type, enum_info_ptr, 1, "variant_index_ptr")
                    .unwrap();
                let idx_val = self.context.i64_type().const_int(idx as u64, false);
                self.builder.build_store(idx_gep, idx_val).unwrap();

                let enum_info_int = self
                    .builder
                    .build_ptr_to_int(enum_info_ptr, self.context.i64_type(), "enum_info_as_int")
                    .unwrap();

                let alloca = self
                    .builder
                    .build_alloca(self.runtime_value_type, &full_name)
                    .unwrap();

                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, alloca, 0, "enum_tag_ptr")
                    .unwrap();
                self.builder.build_store(tag_ptr, enum_tag).unwrap();

                let data_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, alloca, 1, "enum_data_ptr")
                    .unwrap();
                self.builder.build_store(data_ptr, enum_info_int).unwrap();

                self.builder.position_at_end(current_block);
                alloca
            } else {
                let global_name = format!("enum_name_str_{}", full_name.replace(".", "_"));
                let str_const = self.context.const_string(full_name.as_bytes(), true);
                let global_str = module.add_global(
                    str_const.get_type(),
                    Some(AddressSpace::default()),
                    &global_name,
                );
                global_str.set_initializer(&str_const);
                global_str.set_constant(true);
                global_str.set_linkage(Linkage::Internal);

                let zero = self.context.i32_type().const_int(0, false);
                let name_ptr = unsafe {
                    global_str
                        .as_pointer_value()
                        .const_gep(self.context.i8_type(), &[zero, zero])
                };

                let idx_val = self.context.i64_type().const_int(idx as u64, false);
                let enum_info_const =
                    enum_info_type.const_named_struct(&[name_ptr.into(), idx_val.into()]);

                let global_info_name = format!("enum_info_const_{}", full_name.replace(".", "_"));

                let global_enum_info = module.add_global(
                    enum_info_type,
                    Some(AddressSpace::default()),
                    &global_info_name,
                );
                global_enum_info.set_initializer(&enum_info_const);
                global_enum_info.set_constant(true);
                global_enum_info.set_linkage(Linkage::Internal);

                let enum_info_ptr = global_enum_info.as_pointer_value();
                let enum_info_int = enum_info_ptr.const_to_int(self.context.i64_type());

                let global = module.add_global(
                    self.runtime_value_type,
                    Some(AddressSpace::default()),
                    &full_name,
                );
                let const_val = self
                    .runtime_value_type
                    .const_named_struct(&[enum_tag.into(), enum_info_int.into()]);
                global.set_initializer(&const_val);
                global.set_constant(true);
                global.as_pointer_value()
            };

            self.add_variable(full_name, ptr.into(), Type::Enum);
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

    pub fn get_expr_name(&self, expr: &ast::Expr) -> Option<String> {
        match expr {
            ast::Expr::Var(name) => Some(name.clone()),
            _ => None,
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
                if let ast::Expr::Var(name) = lhs.as_ref() {
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

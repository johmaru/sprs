use crate::front::ast;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::span::Spanned;
use crate::front::type_helper;
use crate::front::type_helper::{Type, TypeAnnot};
use crate::llvm::builder_helper;
use crate::llvm::builder_helper::Comparison;
use crate::llvm::builder_helper::EqNeq;
use crate::llvm::builder_helper::UpDown;
use crate::llvm::parser::parse_only;
use crate::naming;
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

/// Sprs-level function signature (not the LLVM ABI).
#[derive(Debug, Clone)]
pub struct FnTypeInfo {
    pub ret_ty: Option<Type>,
    pub params: Vec<Option<TypeAnnot>>,
}

/// Local binding metadata in a scope.
#[derive(Clone)]
pub struct VarBinding<'ctx> {
    pub value: BasicValueEnum<'ctx>,
    pub ty: Type,
    pub always_clone: bool,
    /// Annotated with `ambi` — reassignment may change the static type.
    pub is_ambi: bool,
    /// Came from a type annotation (`>> T` / `>> ambi T`).
    pub is_annotated: bool,
}

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub modules: HashMap<String, Module<'ctx>>, // name, module
    pub builder: Builder<'ctx>,
    pub scopes: Vec<Scope<'ctx>>,
    pub function_signatures: Option<FunctionValue<'ctx>>,
    /// Current function's Sprs return annotation while compiling its body.
    pub current_fn_ret_ty: Option<Type>,
    /// Declared Sprs signatures, keyed by LLVM/function name.
    pub fn_types: HashMap<String, FnTypeInfo>,
    pub runtime_value_type: StructType<'ctx>,
    pub target_os: OS,
    pub string_counter: usize,
    pub malloc_type: inkwell::types::FunctionType<'ctx>,
    pub source_path: String,
    pub struct_defs: HashMap<String, StructDef<'ctx>>, // struct name -> struct definition
    pub enum_names: HashSet<String>,
    pub sources: HashMap<String, String>, // module name → source text
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

pub enum StrConstantResult<'ctx> {
    Global(GlobalValue<'ctx>),
    Pointer(PointerValue<'ctx>),
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
        s: &str,
        is_global: bool,
        is_const: bool,
    ) -> Option<StrConstantResult<'ctx>> {
        let idx = self.string_counter;
        self.string_counter += 1;
        let global_name = if is_global {
            format!("str_const_global_{}", idx)
        } else {
            format!("str_const_const_{}", idx)
        };
        let str_const = self.context.const_string(s.as_bytes(), true);
        let global_str = module.add_global(
            str_const.get_type(),
            Some(AddressSpace::default()),
            global_name.as_str(),
        );
        global_str.set_initializer(&str_const);
        if is_const {
            global_str.set_constant(true);
        }
        global_str.set_linkage(if is_global {
            Linkage::External
        } else {
            Linkage::Internal
        });
        Some(StrConstantResult::Global(global_str))
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum OS {
    Unknown, // default triple
    Windows,
    Linux,
}

/// Runtime value tag stored in `{ i32 tag, i64 data }`.
///
/// Discriminants must stay in sync with [`Type::tag_discriminant`]
/// (`front/type_helper.rs`).
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
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
    Error = 9,

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

impl Tag {
    pub fn from_type(ty: &Type) -> Option<Tag> {
        ty.tag_discriminant().and_then(Self::from_discriminant)
    }

    pub fn to_type(self) -> Type {
        Type::from_tag_discriminant(self as u32).unwrap_or(Type::Any)
    }

    pub fn from_discriminant(disc: u32) -> Option<Tag> {
        match disc {
            0 => Some(Tag::Integer),
            1 => Some(Tag::Float),
            2 => Some(Tag::String),
            3 => Some(Tag::Boolean),
            4 => Some(Tag::List),
            5 => Some(Tag::Range),
            6 => Some(Tag::Unit),
            7 => Some(Tag::Enum),
            8 => Some(Tag::Struct),
            9 => Some(Tag::Error),
            100 => Some(Tag::Int8),
            101 => Some(Tag::Uint8),
            102 => Some(Tag::Int16),
            103 => Some(Tag::Uint16),
            104 => Some(Tag::Int32),
            105 => Some(Tag::Uint32),
            106 => Some(Tag::Int64),
            107 => Some(Tag::Uint64),
            108 => Some(Tag::Float16),
            109 => Some(Tag::Float32),
            110 => Some(Tag::Float64),
            _ => None,
        }
    }
}

pub(crate) const WINDOWS_STR: &str = "Windows";
pub(crate) const LINUX_STR: &str = "Linux";

pub struct Scope<'ctx> {
    pub variables: HashMap<String, VarBinding<'ctx>>,
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
            current_fn_ret_ty: None,
            fn_types: HashMap::new(),
            runtime_value_type,
            target_os: OS::Unknown,
            string_counter: 0,
            malloc_type,
            source_path,
            struct_defs: HashMap::new(),
            enum_names: HashSet::new(),
            sources: HashMap::new(),
        }
    }

    pub(crate) fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    pub(crate) fn exit_scope(&mut self, module: &Module<'ctx>) -> Result<(), SprsError> {
        let scope = self.scopes.pop().unwrap();

        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            let drop_fn = self.get_runtime_fn(module, "__drop")?;

            for name in scope.var_name.iter().rev() {
                if let Some(binding) = scope.variables.get(name) {
                    if binding.value.is_pointer_value() {
                        builder_helper::drop_var(
                            self,
                            binding.value.into_pointer_value(),
                            drop_fn,
                            name,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_variables(&self, name: &str) -> Option<VarBinding<'ctx>> {
        for scope in self.scopes.iter().rev() {
            if let Some(var) = scope.variables.get(name) {
                return Some(var.clone());
            }
        }
        None
    }

    pub fn add_variable(
        &mut self,
        name: String,
        value: BasicValueEnum<'ctx>,
        ty: Type,
        is_clone_variable: bool,
        is_ambi: bool,
        is_annotated: bool,
    ) {
        if let Some(current_scope) = self.scopes.last_mut() {
            current_scope.variables.insert(
                name.clone(),
                VarBinding {
                    value,
                    ty,
                    always_clone: is_clone_variable,
                    is_ambi,
                    is_annotated,
                },
            );
            current_scope.var_name.push(name);
        }
    }

    /// Update the static type of an existing binding (e.g. after `ambi` reassignment).
    pub fn set_variable_type(&mut self, name: &str, ty: Type) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(var) = scope.variables.get_mut(name) {
                var.ty = ty;
                return;
            }
        }
    }

    pub fn remove_variable(&mut self, name: &str) {
        if let Some(current_scope) = self.scopes.last_mut() {
            current_scope.variables.remove(name);
        }
    }

    pub(crate) fn emit_drop_for_return(&mut self, module: &Module<'ctx>) -> Result<(), SprsError> {
        let drop_fn = self.get_runtime_fn(module, "__drop")?;

        let mut vars_to_drop: Vec<(PointerValue<'ctx>, String)> = Vec::new();

        for scope in self.scopes.iter().skip(1).rev() {
            for name in scope.var_name.iter().rev() {
                if let Some(binding) = scope.variables.get(name) {
                    if binding.value.is_pointer_value() {
                        vars_to_drop.push((binding.value.into_pointer_value(), name.clone()));
                    }
                }
            }
        }

        for (ptr, var_name) in vars_to_drop.into_iter() {
            builder_helper::drop_var(self, ptr, drop_fn, &var_name);
        }
        Ok(())
    }

    pub fn register_struct(&mut self, name: String, fields: Vec<ast::StructField>) {
        let mut field_indices = HashMap::new();
        let mut llvm_field_types: Vec<BasicTypeEnum> = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            field_indices.insert(field.ident.clone(), i as u32);

            let llvm_ty = if let Some(ty) = &field.ty {
                match ty {
                    Type::Any
                    | Type::Unit
                    | Type::List
                    | Type::Range
                    | Type::Struct(_)
                    | Type::Error => self.runtime_value_type.into(),
                    Type::Int => self.context.i64_type().into(),
                    Type::Str => self.context.ptr_type(AddressSpace::default()).into(),
                    Type::Float => self.context.f64_type().into(),
                    Type::Bool => self.context.bool_type().into(),
                    Type::Enum(name) => self.context.i64_type().into(),
                    Type::App(_, _) => unreachable!(),
                    Type::Param(_) => unreachable!(),
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

    /// Returns the index of a field in a struct definition.
    ///
    /// rust-analyzer may report `Result<u32, ()>` (E0308) on this function due to
    /// incomplete type resolution of `inkwell`'s FFI types (see `analysis-stats`:
    /// `??ty` unresolved types). `cargo check` passes, so this is a false positive.
    pub fn get_field_index(&self, struct_name: &str, field_name: &str) -> Result<u32, SprsError> {
        self.struct_defs
            .get(struct_name)
            .and_then(|def| def.field_indices.get(field_name).cloned())
            .ok_or_else(|| SprsError::Semantic {
                code: ErrorCode {
                    category: ErrorCategory::Semantic,
                    number: 7,
                },
                location: Location::new(String::new(), Span::DUMMY),
                message: format!(
                    "Field '{}' not found in struct '{}'",
                    field_name, struct_name
                ),
                help: None,
            })
    }

    pub fn build_list_from_exprs(
        &mut self,
        elements: &[Spanned<ast::Expr>],
        module: &Module<'ctx>,
    ) -> Result<IntValue<'ctx>, SprsError> {
        let create = builder_helper::create_list_from_expr(self, elements, module);
        create
    }

    pub fn get_runtime_fn(
        &self,
        module: &Module<'ctx>,
        name: &str,
    ) -> Result<FunctionValue<'ctx>, SprsError> {
        if let Some(func) = module.get_function(name) {
            return Ok(func);
        }

        let i64_type = self.context.i64_type();
        let i32_type = self.context.i32_type();
        let i8_ptr_type = self.context.ptr_type(inkwell::AddressSpace::default());
        let void_type = self.context.void_type();

        let fn_type = match name {
            // slab (handle-based) runtime ABI:
            //   list/range/string/struct/enum values are addressed by a u64
            //   handle packed as (index:u32 | generation:u32). Primitives
            //   (Integer/Float/Bool/cast! types) carry their value inline in
            //   `data` and are not slab-backed.
            "__list_new" => i64_type.fn_type(&[i64_type.into()], false),
            "__list_push" => void_type.fn_type(
                &[
                    i64_type.into(), // list handle
                    i32_type.into(), // value tag
                    i64_type.into(), // value data (handle or immediate)
                ],
                false,
            ),
            "__list_get" => self.runtime_value_type.fn_type(
                &[
                    i64_type.into(), // list handle
                    i64_type.into(), // index
                ],
                false,
            ),
            "__range_new" => i64_type.fn_type(
                &[
                    i64_type.into(), // start
                    i64_type.into(), // end
                ],
                false,
            ),
            "__println" => void_type.fn_type(&[i64_type.into()], false),
            "__strlen" => i64_type.fn_type(&[i64_type.into()], false),
            "__malloc" => i8_ptr_type.fn_type(&[i64_type.into()], false),
            "__drop" => void_type.fn_type(&[i32_type.into(), i64_type.into()], false),
            "__clone" => self.runtime_value_type.fn_type(
                &[
                    i32_type.into(), // value tag
                    i64_type.into(), // value data
                ],
                false,
            ),
            "__error_new" => i64_type.fn_type(
                &[
                    i32_type.into(),    // error code
                    i8_ptr_type.into(), // message ptr (may be null)
                    i64_type.into(),    // message length
                ],
                false,
            ),
            "__is_error" => i32_type.fn_type(
                &[i64_type.into()], // slab handle (data field)
                false,
            ),
            "__error_code" => i32_type.fn_type(
                &[i64_type.into()], // slab handle
                false,
            ),
            "__error_message" => i64_type.fn_type(
                &[i64_type.into()], // slab handle
                false,
            ),
            "__panic" => void_type.fn_type(&[i8_ptr_type.into()], false),
            // String slot construction (replaces inline global-pointer storage
            // so the slot owns a proper Rust String with length tracking).
            "__string_new" => i64_type.fn_type(
                &[
                    i8_ptr_type.into(), // bytes pointer
                    i64_type.into(),    // length
                ],
                false,
            ),
            "__string_from_cstr" => i64_type.fn_type(&[i8_ptr_type.into()], false),
            "__string_concat" => i64_type.fn_type(
                &[
                    i64_type.into(), // left handle
                    i64_type.into(), // right handle
                ],
                false,
            ),
            // Struct slot construction (runtime owns the allocation).
            "__struct_new" => i64_type.fn_type(&[i64_type.into()], false),
            "__struct_borrow" => i8_ptr_type.fn_type(&[i64_type.into()], false),
            // Enum slot construction.
            "__enum_new" => i64_type.fn_type(
                &[
                    i8_ptr_type.into(), // variant name bytes
                    i64_type.into(),    // name length
                    i64_type.into(),    // variant index
                ],
                false,
            ),
            _ => {
                return Err(SprsError::Semantic {
                    code: ErrorCode {
                        category: ErrorCategory::Semantic,
                        number: 6,
                    },
                    location: Location::new(String::new(), Span::DUMMY),
                    message: format!("Unknown runtime function: {}", name),
                    help: None,
                });
            }
        };

        Ok(module.add_function(name, fn_type, None))
    }
}

#[cfg(test)]
mod tag_type_sync_tests {
    use super::Tag;
    use crate::front::type_helper::Type;

    #[test]
    fn type_and_tag_discriminants_stay_aligned() {
        let cases: &[(Type, Tag)] = &[
            (Type::Int, Tag::Integer),
            (Type::Float, Tag::Float),
            (Type::Str, Tag::String),
            (Type::Bool, Tag::Boolean),
            (Type::List, Tag::List),
            (Type::Range, Tag::Range),
            (Type::Unit, Tag::Unit),
            (Type::Enum("Point".into()), Tag::Enum),
            (Type::Struct("Point".into()), Tag::Struct),
            (Type::Error, Tag::Error),
            (Type::TypeI8, Tag::Int8),
            (Type::TypeU8, Tag::Uint8),
            (Type::TypeI16, Tag::Int16),
            (Type::TypeU16, Tag::Uint16),
            (Type::TypeI32, Tag::Int32),
            (Type::TypeU32, Tag::Uint32),
            (Type::TypeI64, Tag::Int64),
            (Type::TypeU64, Tag::Uint64),
            (Type::TypeF16, Tag::Float16),
            (Type::TypeF32, Tag::Float32),
            (Type::TypeF64, Tag::Float64),
        ];

        for (ty, tag) in cases {
            assert_eq!(
                ty.tag_discriminant(),
                Some(*tag as u32),
                "Type::{ty:?} discriminant mismatch"
            );
            assert_eq!(Tag::from_type(ty), Some(*tag));
            assert_eq!(tag.to_type().tag_discriminant(), Some(*tag as u32));
        }

        assert_eq!(Type::Any.tag_discriminant(), None);
        assert_eq!(Tag::from_type(&Type::Any), None);
    }

    #[test]
    fn sprs_return_allows_declared_type_or_error() {
        // Mirrors validate_sprs_return_type rules without needing LLVM.
        fn ok(expected: Option<Type>, actual: Type) -> bool {
            match expected {
                None => true,
                Some(exp) => actual == Type::Any || actual == Type::Error || actual == exp,
            }
        }
        assert!(ok(Some(Type::List), Type::List));
        assert!(ok(Some(Type::List), Type::Error));
        assert!(ok(Some(Type::List), Type::Any));
        assert!(!ok(Some(Type::List), Type::Int));
        assert!(ok(None, Type::Int));
    }
}

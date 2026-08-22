use crate::front::hir;
use crate::front::ast;
use crate::front::ast::FbCondition;
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::type_helper::Type;
use crate::llvm::builder_helper;
use inkwell::AddressSpace;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::BasicTypeEnum;
use inkwell::types::StructType;
use inkwell::values::GlobalValue;
use inkwell::values::IntValue;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use std::collections::HashMap;
use std::collections::HashSet;

pub struct StructDef<'ctx> {
    pub fields: Vec<ast::StructField>,
    pub llvm_type: StructType<'ctx>,
}

/// Local binding metadata in a scope.
#[derive(Clone)]
pub struct VarBinding<'ctx> {
    pub value: BasicValueEnum<'ctx>,
}

pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub modules: HashMap<String, Module<'ctx>>, // name, module
    pub builder: Builder<'ctx>,
    pub scopes: Vec<Scope<'ctx>>,
    pub function_signatures: Option<FunctionValue<'ctx>>,
    pub runtime_value_type: StructType<'ctx>,
    pub target_os: OS,
    pub string_counter: usize,
    pub source_path: String,
    /// Absolute/relative path of the module currently being compiled.
    /// Used for type/semantic error locations (was previously left empty).
    pub current_file: String,
    pub struct_defs: HashMap<String, StructDef<'ctx>>, // struct name -> struct definition
    pub closed_label_sets: HashSet<String>,
    /// FunctionBuild type parameters and `when` rules, keyed by build name.
    /// Filled from the registry during module loading so prototype declaration
    /// and call-site resolution can reuse the resolved contract.
    pub function_build_contracts: HashMap<String, (Vec<String>, Vec<(FbCondition, Type)>)>,
    /// Members of non-public closed label sets, filled after the defining
    /// module's functions are compiled so same-module functions still see them.
    pub private_closed_label_members: HashSet<String>,
    /// Standalone `label :name;` declarations (module-global Atom constants).
    pub atom_defs: HashSet<String>,
    /// Non-public atom names, filled after the defining module compiles.
    pub private_atom_defs: HashSet<String>,
    pub sources: HashMap<String, String>, // module name → source text
    /// Values captured by @attach within the current function.
    pub attachments: HashMap<String, PointerValue<'ctx>>,
    pub hir_modules: HashMap<String, crate::front::hir::Module>,
    pub typecheck_visiting: Vec<String>,
}

pub enum StoreTag<'ctx> {
    Int(u64),
    Dynamic(IntValue<'ctx>),
}

pub enum StoreValue<'ctx> {
    Int(IntValue<'ctx>),
    Float(f64),
    Bool(IntValue<'ctx>),
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
            StoreTag::Int(tag_value) => self.context.i32_type().const_int(tag_value, false),
            StoreTag::Dynamic(tag_value) => tag_value,
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
            StoreValue::Int(stored_value) => stored_value,
            StoreValue::Float(float_value) => self
                .context
                .i64_type()
                .const_int(float_value.to_bits(), false),
            StoreValue::Bool(boolean_value) => self
                .builder
                .build_int_z_extend(boolean_value, self.context.i64_type(), name)
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
        source_text: &str,
        is_global: bool,
        is_const: bool,
    ) -> GlobalValue<'ctx> {
        let idx = self.string_counter;
        self.string_counter += 1;
        let global_name = if is_global {
            format!("str_const_global_{}", idx)
        } else {
            format!("str_const_const_{}", idx)
        };
        let str_const = self.context.const_string(source_text.as_bytes(), true);
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
        global_str
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
    Struct = 8,
    Atom = 9, // immediate: data = interned atom id (u32 as u64). NOT a slab handle
    Label = 10,
    Buffer = 11,
    RawPtr = 12, // bare address in `data`; not a slab handle

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

pub(crate) const WINDOWS_STR: &str = "Windows";
pub(crate) const LINUX_STR: &str = "Linux";

pub struct Scope<'ctx> {
    pub variables: HashMap<String, VarBinding<'ctx>>,
    pub var_name: Vec<String>,
    /// `defer <expr>;` queue; run LIFO at scope exit, before variable `__drop`.
    pub deferred: Vec<hir::Expr>,
}

impl<'ctx> Scope<'ctx> {
    pub fn new() -> Self {
        Scope {
            variables: HashMap::new(),
            var_name: Vec::new(),
            deferred: Vec::new(),
        }
    }
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, builder: Builder<'ctx>, source_path: String) -> Self {
        let runtime_value_type = context.struct_type(
            &[context.i32_type().into(), context.i64_type().into()],
            false,
        );

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
            string_counter: 0,
            source_path,
            current_file: String::new(),
            struct_defs: HashMap::new(),
            closed_label_sets: HashSet::new(),
            function_build_contracts: HashMap::new(),
            private_closed_label_members: HashSet::new(),
            atom_defs: HashSet::new(),
            private_atom_defs: HashSet::new(),
            sources: HashMap::new(),
            attachments: HashMap::new(),
            hir_modules: HashMap::new(),
            typecheck_visiting: Vec::new(),
        }
    }

    pub(crate) fn location(&self, span: crate::front::span::Span) -> Location {
        Location::new(self.current_file.clone(), span)
    }

    pub(crate) fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    pub(crate) fn exit_scope(&mut self, module: &Module<'ctx>) -> Result<(), SprsError> {
        let mut scope = self.scopes.pop().unwrap();

        if self
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            let drop_fn = self.get_runtime_fn(module, "__drop")?;

            // Take deferred first: compile_expr needs &mut self, and must run before drops.
            let deferred = std::mem::take(&mut scope.deferred);
            for expr in deferred.iter().rev() {
                self.compile_expr(expr, module)?;
            }

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
    ) {
        if let Some(current_scope) = self.scopes.last_mut() {
            current_scope.variables.insert(
                name.clone(),
                VarBinding { value },
            );
            current_scope.var_name.push(name);
        }
    }

    pub fn remove_variable(&mut self, name: &str) {
        if let Some(current_scope) = self.scopes.last_mut() {
            current_scope.variables.remove(name);
        }
    }

    pub(crate) fn emit_drop_for_return(&mut self, module: &Module<'ctx>) -> Result<(), SprsError> {
        let drop_fn = self.get_runtime_fn(module, "__drop")?;

        let mut scope_work: Vec<(Vec<hir::Expr>, Vec<(PointerValue<'ctx>, String)>)> =
            Vec::new();
        // skip(1): exclude scopes[0] (global). Remaining scopes, including the
        // function-argument scope, are cleaned up innermost-first. Deferred/vars
        // are taken first so compile_expr does not borrow `scopes` while mutating.
        for scope in self.scopes.iter_mut().skip(1) {
            let deferred = std::mem::take(&mut scope.deferred);
            let mut vars = Vec::new();
            for name in scope.var_name.iter().rev() {
                if let Some(binding) = scope.variables.get(name) {
                    if binding.value.is_pointer_value() {
                        vars.push((binding.value.into_pointer_value(), name.clone()));
                    }
                }
            }
            scope_work.push((deferred, vars));
        }

        for (deferred, vars) in scope_work.into_iter().rev() {
            for expr in deferred.iter().rev() {
                self.compile_expr(expr, module)?;
            }
            for (ptr, var_name) in vars.into_iter() {
                builder_helper::drop_var(self, ptr, drop_fn, &var_name);
            }
        }
        self.emit_drop_for_attachments(module)?;
        Ok(())
    }

    pub(crate) fn emit_drop_for_attachments(
        &mut self,
        module: &Module<'ctx>,
    ) -> Result<(), SprsError> {
        let drop_fn = self.get_runtime_fn(module, "__drop")?;
        let attachments: Vec<(String, PointerValue<'ctx>)> = self
            .attachments
            .iter()
            .map(|(name, ptr)| (name.clone(), *ptr))
            .collect();
        for (name, ptr) in attachments {
            builder_helper::drop_var(self, ptr, drop_fn, &format!("attach_{}", name));
        }
        Ok(())
    }

    pub fn register_struct(
        &mut self,
        name: String,
        fields: Vec<ast::StructField>,
    ) -> Result<(), SprsError> {
        let mut llvm_field_types: Vec<BasicTypeEnum> = Vec::new();
        for field in fields.iter() {
            let llvm_ty = if let Some(ty) = &field.ty {
                match ty {
                    Type::Any
                    | Type::Unit
                    | Type::Range
                    | Type::Struct(_)
                    | Type::Label
                    | Type::AtomVal
                    | Type::ClosedLabelSet(_)
                    | Type::Buffer
                    | Type::RawPtr
                    | Type::App(_, _)
                    | Type::Param(_)
                    | Type::Atom(_) => self.runtime_value_type.into(),
                    Type::Named(_) | Type::SelfType => self.runtime_value_type.into(),
                    Type::Int
                    | Type::TypeI64
                    | Type::TypeU64
                    | Type::Bool
                    | Type::Str
                    | Type::Float
                    | Type::TypeF64
                    | Type::TypeI8
                    | Type::TypeU8
                    | Type::TypeI16
                    | Type::TypeU16
                    | Type::TypeI32
                    | Type::TypeU32
                    | Type::TypeF16
                    | Type::TypeF32 => self.context.i64_type().into(),
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
                llvm_type,
            },
        );
        Ok(())
    }

    pub fn build_list_from_exprs(
        &mut self,
        elements: &[hir::Expr],
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
            "__list_set" => void_type.fn_type(
                &[
                    i64_type.into(), // list handle
                    i64_type.into(), // index
                    i32_type.into(), // value tag
                    i64_type.into(), // value data
                ],
                false,
            ),
            // Buffer slot construction (fixed-size byte array via `new(n)`).
            "__buffer_new" => i64_type.fn_type(&[i64_type.into()], false),
            "__buffer_len" => i64_type.fn_type(&[i64_type.into()], false),
            "__buffer_get" => self.runtime_value_type.fn_type(
                &[
                    i64_type.into(), // buffer handle
                    i64_type.into(), // index
                ],
                false,
            ),
            "__buffer_set" => void_type.fn_type(
                &[
                    i64_type.into(), // buffer handle
                    i64_type.into(), // index
                    i64_type.into(), // byte value
                ],
                false,
            ),
            "__buffer_exist" => i32_type.fn_type(&[i64_type.into()], false),
            "__buffer_into_raw" => i64_type.fn_type(&[i64_type.into()], false),
            "__raw_free" => void_type.fn_type(&[i64_type.into()], false),
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
            "__atom_from_bytes" => i64_type.fn_type(
                &[
                    i8_ptr_type.into(), // atom name bytes
                    i64_type.into(),    // atom name length
                ],
                false,
            ),
            "__atom_from_string" => i64_type.fn_type(
                &[i64_type.into()], // string handle
                false,
            ),
            "__atom_name" => i64_type.fn_type(
                &[i64_type.into()], // atom id
                false,
            ),
            "__atom_eq" => i32_type.fn_type(
                &[
                    i64_type.into(), // atom id a
                    i64_type.into(), // atom id b
                ],
                false,
            ),
            "__label_new" => i64_type.fn_type(
                &[
                    i8_ptr_type.into(), // label name bytes
                    i64_type.into(),    // label name length
                    i32_type.into(),    // payload tag
                    i64_type.into(),    // payload data
                ],
                false,
            ),
            "__value_to_string" => i64_type.fn_type(
                &[
                    i32_type.into(), // value tag
                    i64_type.into(), // value data
                ],
                false,
            ),
            "__label_new_from_string" => i64_type.fn_type(
                &[
                    i64_type.into(), // name string handle
                    i32_type.into(), // payload tag
                    i64_type.into(), // payload data
                ],
                false,
            ),
            "__label_name_eq" => i32_type.fn_type(
                &[
                    i64_type.into(),    // label handle
                    i8_ptr_type.into(), // expected name bytes
                    i64_type.into(),    // expected name length
                ],
                false,
            ),
            "__label_names_equal" => i32_type.fn_type(
                &[
                    i64_type.into(), // label handle a
                    i64_type.into(), // label handle b
                ],
                false,
            ),
            "__label_payload" => self.runtime_value_type.fn_type(
                &[i64_type.into()], // label handle
                false,
            ),
            "__label_name" => i64_type.fn_type(
                &[i64_type.into()], // label handle
                false,
            ),
            "__label_is_error" => i32_type.fn_type(
                &[
                    i32_type.into(), // value tag
                    i64_type.into(), // value data
                ],
                false,
            ),
            "__error_label_from_str" => i64_type.fn_type(
                &[
                    i8_ptr_type.into(), // reason bytes
                    i64_type.into(),    // reason length
                ],
                false,
            ),
            "__error_message_from_label" => i64_type.fn_type(
                &[i64_type.into()], // label handle
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
            "__string_eq" => i32_type.fn_type(
                &[
                    i64_type.into(), // left handle
                    i64_type.into(), // right handle
                ],
                false,
            ),
            // Struct slot construction (runtime owns the allocation).
            "__struct_new" => i64_type.fn_type(&[i64_type.into()], false),
            "__struct_borrow" => i8_ptr_type.fn_type(&[i64_type.into()], false),
            "__struct_track_value" => i32_type.fn_type(
                &[
                    i64_type.into(),    // struct handle
                    i8_ptr_type.into(), // field pointer
                    i32_type.into(),    // value tag
                    i64_type.into(),    // value data
                    i32_type.into(),    // data_only
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

//! Concrete `StorageRep(T)` layout.
//!
//! LLVM target ABI / data layout is the only source of truth for size,
//! alignment, and struct padding. RuntimeValue `{tag,data}` is not used as
//! pointer stride or storage layout.

use inkwell::AddressSpace;
use inkwell::OptimizationLevel;
use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::types::{AnyType, AnyTypeEnum, BasicTypeEnum};
use inkwell::values::IntValue;

use crate::front::error::SprsError;
use crate::front::span::Span;
use crate::front::type_helper::{Type, is_handle_type, maybe_uninit_inner};
use crate::llvm::compiler::Compiler;

#[derive(Clone, Copy, Debug)]
pub struct TypeLayout<'ctx> {
    pub llvm_type: BasicTypeEnum<'ctx>,
    pub size: u64,
    #[allow(dead_code)]
    pub align: u32,
}

pub fn host_target_machine() -> TargetMachine {
    create_target_machine(&TargetMachine::get_default_triple())
}

/// Object codegen and layout share one TargetMachine. Opt level does not
/// change TargetData size/align; keep Default so final objects are not None.
pub fn target_opt_level() -> OptimizationLevel {
    OptimizationLevel::Default
}

pub fn create_target_machine(triple: &TargetTriple) -> TargetMachine {
    let _ = Target::initialize_native(&InitializationConfig::default());
    Target::initialize_x86(&InitializationConfig::default());
    let target = Target::from_triple(triple).unwrap_or_else(|err| {
        panic!("failed to resolve LLVM target {triple}: {err}");
    });
    target
        .create_target_machine(
            triple,
            "generic",
            "",
            target_opt_level(),
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap_or_else(|| panic!("failed to create LLVM target machine for {triple}"))
}

pub fn unwrap_storage_type(ty: &Type) -> Type {
    let mut current = ty.clone();
    while let Some(inner) = maybe_uninit_inner(&current) {
        current = inner.clone();
    }
    current
}

fn layout_error(message: impl Into<String>) -> SprsError {
    SprsError::Internal {
        message: message.into(),
        location: None,
    }
}

fn is_user_struct_app(ty: &Type) -> bool {
    match ty {
        Type::App(name, _) => !matches!(
            name.as_str(),
            "List" | "Ptr" | "MaybeUninit" | "Process" | "Label"
        ),
        _ => false,
    }
}

fn layout_symbol(ty: &Type) -> String {
    let rendered = ty.to_string();
    let mut out = String::from("sprs.storage.");
    for ch in rendered.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

impl<'ctx> Compiler<'ctx> {
    pub fn storage_layout(&mut self, ty: &Type) -> Result<TypeLayout<'ctx>, SprsError> {
        let storage_ty = unwrap_storage_type(ty);
        if let Some(layout) = self.layout_cache.get(&storage_ty).copied() {
            return Ok(layout);
        }
        let layout = self.compute_storage_layout(&storage_ty)?;
        self.layout_cache.insert(storage_ty, layout);
        Ok(layout)
    }

    pub fn storage_stride_const(&mut self, ty: &Type) -> Result<IntValue<'ctx>, SprsError> {
        let layout = self.storage_layout(ty)?;
        Ok(self.context.i64_type().const_int(layout.size, false))
    }

    fn compute_storage_layout(&mut self, ty: &Type) -> Result<TypeLayout<'ctx>, SprsError> {
        let llvm_type = self.storage_llvm_type(ty)?;
        let data = self.target_machine.get_target_data();
        let any_ty: AnyTypeEnum = llvm_type.as_any_type_enum();
        Ok(TypeLayout {
            llvm_type,
            size: data.get_store_size(&any_ty),
            align: data.get_abi_alignment(&any_ty),
        })
    }

    pub fn storage_llvm_type(&mut self, ty: &Type) -> Result<BasicTypeEnum<'ctx>, SprsError> {
        let ty = unwrap_storage_type(ty);
        if let Some(layout) = self.layout_cache.get(&ty).copied() {
            return Ok(layout.llvm_type);
        }
        match &ty {
            Type::TypeI8 | Type::TypeU8 | Type::Bool => Ok(self.context.i8_type().into()),
            Type::TypeI16 | Type::TypeU16 | Type::TypeF16 => Ok(self.context.i16_type().into()),
            Type::TypeI32 | Type::TypeU32 => Ok(self.context.i32_type().into()),
            Type::TypeF32 => Ok(self.context.f32_type().into()),
            Type::Int | Type::TypeI64 | Type::TypeU64 | Type::TypeF64 | Type::Float => {
                if matches!(ty, Type::Float | Type::TypeF64) {
                    Ok(self.context.f64_type().into())
                } else {
                    Ok(self.context.i64_type().into())
                }
            }
            Type::TypeUsize => {
                let data = self.target_machine.get_target_data();
                Ok(self.context.ptr_sized_int_type(&data, None).into())
            }
            Type::RawPtr => Ok(self.context.ptr_type(AddressSpace::default()).into()),
            Type::App(name, args) if name == "Ptr" && args.len() == 1 => {
                Ok(self.context.ptr_type(AddressSpace::default()).into())
            }
            Type::Unit => Ok(self.context.struct_type(&[], false).into()),
            Type::Label | Type::Any => Ok(self.runtime_value_type.into()),
            Type::Struct(_) => self.struct_storage_llvm_type(&ty),
            other if is_handle_type(other) => Ok(self.context.i64_type().into()),
            other if is_user_struct_app(other) => self.struct_storage_llvm_type(other),
            Type::Named(_) | Type::SelfType | Type::Param(_) => Err(layout_error(format!(
                "no concrete StorageRep for unresolved type {ty}"
            ))),
            other => Err(layout_error(format!(
                "no concrete StorageRep for type {other}"
            ))),
        }
    }

    fn struct_storage_llvm_type(&mut self, ty: &Type) -> Result<BasicTypeEnum<'ctx>, SprsError> {
        if let Some(layout) = self.layout_cache.get(ty).copied() {
            return Ok(layout.llvm_type);
        }
        let fields = self.struct_storage_fields(ty)?;
        let opaque_name = layout_symbol(ty);
        let struct_ty = if let Some(existing) = self.context.get_struct_type(&opaque_name) {
            existing
        } else {
            self.context.opaque_struct_type(&opaque_name)
        };
        let placeholder = TypeLayout {
            llvm_type: struct_ty.into(),
            size: 0,
            align: 1,
        };
        self.layout_cache.insert(ty.clone(), placeholder);
        let mut field_tys = Vec::with_capacity(fields.len());
        for (_, field_ty) in &fields {
            field_tys.push(self.storage_llvm_type(field_ty)?);
        }
        if struct_ty.is_opaque() {
            struct_ty.set_body(&field_tys, false);
        }
        let data = self.target_machine.get_target_data();
        let layout = TypeLayout {
            llvm_type: struct_ty.into(),
            size: data.get_store_size(&struct_ty.as_any_type_enum()),
            align: data.get_abi_alignment(&struct_ty.as_any_type_enum()),
        };
        self.layout_cache.insert(ty.clone(), layout);
        Ok(struct_ty.into())
    }

    pub fn struct_storage_fields(&self, ty: &Type) -> Result<Vec<(String, Type)>, SprsError> {
        match ty {
            Type::Struct(name) => self.fields_from_struct_def(name),
            Type::App(name, args) => {
                if let Some(backend) = self.backend_struct_name_for_app(name, args) {
                    return self.fields_from_struct_def(&backend);
                }
                if let Some(fields) = self.fields_from_specialization(name, args) {
                    return Ok(fields);
                }
                Err(layout_error(format!(
                    "missing concrete struct layout for {ty}"
                )))
            }
            other => Err(layout_error(format!(
                "expected struct StorageRep, got {other}"
            ))),
        }
    }

    fn fields_from_struct_def(&self, name: &str) -> Result<Vec<(String, Type)>, SprsError> {
        let def = self
            .struct_defs
            .get(name)
            .ok_or_else(|| layout_error(format!("undefined struct `{name}` for StorageRep")))?;
        let mut fields = Vec::new();
        for field in &def.fields {
            let ty = field.ty.clone().ok_or_else(|| {
                layout_error(format!(
                    "struct `{name}` field `{}` has no type",
                    field.ident
                ))
            })?;
            fields.push((field.ident.clone(), ty));
        }
        Ok(fields)
    }

    fn backend_struct_name_for_app(&self, name: &str, args: &[Type]) -> Option<String> {
        self.struct_specialization_names
            .iter()
            .find(|(id, _)| id.declaration.name == name && id.args == args)
            .map(|(_, backend)| backend.clone())
    }

    fn fields_from_specialization(&self, name: &str, args: &[Type]) -> Option<Vec<(String, Type)>> {
        for module in self.hir_modules.values() {
            for spec in &module.struct_specializations {
                if spec.id.declaration.name == name && spec.id.args == args {
                    return Some(
                        spec.fields
                            .iter()
                            .map(|field| (field.name.clone(), field.ty.clone()))
                            .collect(),
                    );
                }
            }
        }
        None
    }
}

#[allow(dead_code)]
pub fn dummy_span() -> Span {
    Span::DUMMY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::ast::StructField;
    use crate::front::hir::{
        StructField as HirStructField, StructId, StructInstanceId, StructSpecialization,
    };
    use crate::front::type_helper::{maybe_uninit_type, ptr_type};
    use inkwell::OptimizationLevel;
    use inkwell::context::Context;

    fn field(name: &str, ty: Type) -> StructField {
        StructField {
            ident: name.into(),
            ty: Some(ty),
            default_value: None,
            span: Span::DUMMY,
        }
    }

    fn layout_compiler<'ctx>(context: &'ctx Context) -> Compiler<'ctx> {
        let builder = context.create_builder();
        Compiler::new(context, builder, "layout.sprs".into())
    }

    #[test]
    fn primitive_storage_matches_llvm_abi() {
        let context = Context::create();
        let mut compiler = layout_compiler(&context);
        let i8_layout = compiler.storage_layout(&Type::TypeI8).unwrap();
        assert_eq!(i8_layout.size, 1);
        assert_eq!(i8_layout.align, 1);
        let i32_layout = compiler.storage_layout(&Type::TypeI32).unwrap();
        assert_eq!(i32_layout.size, 4);
        assert_eq!(i32_layout.align, 4);
        let i64_layout = compiler.storage_layout(&Type::TypeI64).unwrap();
        assert_eq!(i64_layout.size, 8);
        let usize_layout = compiler.storage_layout(&Type::TypeUsize).unwrap();
        assert_eq!(usize_layout.size, 8);
        let mu = compiler
            .storage_layout(&maybe_uninit_type(Type::TypeI64))
            .unwrap();
        assert_eq!(mu.size, i64_layout.size);
        assert_eq!(mu.align, i64_layout.align);
        assert_eq!(
            compiler
                .storage_layout(&ptr_type(maybe_uninit_type(Type::TypeI64)))
                .unwrap()
                .size,
            compiler
                .storage_layout(&ptr_type(Type::TypeI64))
                .unwrap()
                .size
        );
    }

    #[test]
    fn padded_struct_uses_target_abi() {
        let context = Context::create();
        let mut compiler = layout_compiler(&context);
        compiler
            .register_struct(
                "Foo".into(),
                vec![field("a", Type::TypeI8), field("b", Type::TypeI64)],
            )
            .unwrap();
        let layout = compiler
            .storage_layout(&Type::Struct("Foo".into()))
            .unwrap();
        assert_eq!(layout.size, 16);
        assert_eq!(layout.align, 8);
        let llvm_ty = layout.llvm_type.into_struct_type();
        let data = compiler.target_machine.get_target_data();
        assert_eq!(data.offset_of_element(&llvm_ty, 0).unwrap(), 0);
        assert_eq!(data.offset_of_element(&llvm_ty, 1).unwrap(), 8);
    }

    #[test]
    fn generic_pair_specializations_have_distinct_layouts() {
        let context = Context::create();
        let mut compiler = layout_compiler(&context);
        let pair_i32 = Type::App("Pair".into(), vec![Type::TypeI32]);
        let pair_i64 = Type::App("Pair".into(), vec![Type::TypeI64]);
        compiler
            .ensure_struct_specialization(&StructSpecialization {
                id: StructInstanceId {
                    declaration: StructId {
                        module: "test".into(),
                        name: "Pair".into(),
                    },
                    args: vec![Type::TypeI32],
                },
                type_bindings: vec![("T".into(), Type::TypeI32)],
                fields: vec![
                    HirStructField {
                        name: "a".into(),
                        ty: Type::TypeI32,
                        default_value: None,
                        span: Span::DUMMY,
                    },
                    HirStructField {
                        name: "b".into(),
                        ty: Type::TypeI32,
                        default_value: None,
                        span: Span::DUMMY,
                    },
                ],
                span: Span::DUMMY,
            })
            .unwrap();
        compiler
            .ensure_struct_specialization(&StructSpecialization {
                id: StructInstanceId {
                    declaration: StructId {
                        module: "test".into(),
                        name: "Pair".into(),
                    },
                    args: vec![Type::TypeI64],
                },
                type_bindings: vec![("T".into(), Type::TypeI64)],
                fields: vec![
                    HirStructField {
                        name: "a".into(),
                        ty: Type::TypeI64,
                        default_value: None,
                        span: Span::DUMMY,
                    },
                    HirStructField {
                        name: "b".into(),
                        ty: Type::TypeI64,
                        default_value: None,
                        span: Span::DUMMY,
                    },
                ],
                span: Span::DUMMY,
            })
            .unwrap();
        let i32_layout = compiler.storage_layout(&pair_i32).unwrap();
        let i64_layout = compiler.storage_layout(&pair_i64).unwrap();
        assert_eq!(i32_layout.size, 8);
        assert_eq!(i32_layout.align, 4);
        assert_eq!(i64_layout.size, 16);
        assert_eq!(i64_layout.align, 8);
        assert_ne!(i32_layout.size, i64_layout.size);
    }

    #[test]
    fn owned_handle_and_inline_struct_layouts() {
        let context = Context::create();
        let mut compiler = layout_compiler(&context);
        let str_layout = compiler.storage_layout(&Type::Str).unwrap();
        assert_eq!(str_layout.size, 8);
        compiler
            .register_struct(
                "User".into(),
                vec![field("id", Type::TypeI32), field("name", Type::Str)],
            )
            .unwrap();
        let user = compiler
            .storage_layout(&Type::Struct("User".into()))
            .unwrap();
        assert!(user.size >= 12);
        assert_eq!(user.align, 8);
        let llvm_ty = user.llvm_type.into_struct_type();
        assert_eq!(llvm_ty.count_fields(), 2);
    }

    #[test]
    fn broad_label_storage_keeps_tag_and_data() {
        let context = Context::create();
        let mut compiler = layout_compiler(&context);
        let label = compiler.storage_layout(&Type::Label).unwrap();
        assert_eq!(label.size, 16);
        assert_eq!(label.align, 8);
        let mu = compiler
            .storage_layout(&maybe_uninit_type(Type::Label))
            .unwrap();
        assert_eq!(mu.size, label.size);
        assert_eq!(mu.align, label.align);
        let any = compiler.storage_layout(&Type::Any).unwrap();
        assert_eq!(any.size, label.size);
        assert_eq!(any.align, label.align);
        let mu_any = compiler
            .storage_layout(&maybe_uninit_type(Type::Any))
            .unwrap();
        assert_eq!(mu_any.size, any.size);
        assert_eq!(compiler.storage_layout(&Type::AtomVal).unwrap().size, 8);
        assert_eq!(
            compiler
                .storage_layout(&Type::App(
                    "Label".into(),
                    vec![Type::Atom("ok".into()), Type::TypeI64]
                ))
                .unwrap()
                .size,
            8
        );
    }

    #[test]
    fn object_codegen_uses_default_optimization() {
        assert_eq!(target_opt_level(), OptimizationLevel::Default);
        assert_ne!(target_opt_level(), OptimizationLevel::None);
    }

    #[test]
    fn storage_layout_uses_compile_target_machine() {
        let context = Context::create();
        let mut compiler = layout_compiler(&context);
        compiler.set_compile_target(crate::llvm::compiler::OS::Windows);
        let windows_triple = format!("{:?}", compiler.target_machine.get_triple());
        assert!(
            windows_triple.contains("x86_64-pc-windows-msvc"),
            "{windows_triple}"
        );
        let usize_layout = compiler.storage_layout(&Type::TypeUsize).unwrap();
        let data = compiler.target_machine.get_target_data();
        let ptr_int = compiler.context.ptr_sized_int_type(&data, None);
        assert_eq!(
            usize_layout.size,
            data.get_store_size(&ptr_int.as_any_type_enum())
        );
        let module = context.create_module("target_layout");
        compiler.apply_module_target(&module);
        assert_eq!(
            format!("{}", module.get_triple()),
            format!("{}", compiler.target_machine.get_triple())
        );
        compiler.storage_layout(&Type::TypeI64).unwrap();
        assert!(!compiler.layout_cache.is_empty());
        compiler.set_compile_target(crate::llvm::compiler::OS::Linux);
        assert!(compiler.layout_cache.is_empty());
        let linux_triple = format!("{:?}", compiler.target_machine.get_triple());
        assert!(
            linux_triple.contains("x86_64-pc-linux-gnu"),
            "{linux_triple}"
        );
    }
}

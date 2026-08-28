//! RuntimeValue ↔ StorageRep conversions for typed pointer storage.

use inkwell::AddressSpace;
use inkwell::module::Module;
use inkwell::types::{AnyType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, IntValue, PointerValue, ValueKind};

use crate::front::error::SprsError;
use crate::front::hir;
use crate::front::type_helper::{Type, is_tagged_storage, is_user_struct_type, ptr_element};
use crate::llvm::builder_helper::BuilderExt;
use crate::llvm::compiler::{Compiler, StoreTag, StoreValue, Tag};
use crate::llvm::layout::unwrap_storage_type;
use crate::llvm::value::create_entry_block_alloca;
use crate::llvm::variable::{clone_runtime_value, drop_var};

#[derive(Clone, Copy)]
pub enum StorageLoad {
    Clone,
    Move,
}

fn storage_error(message: impl Into<String>) -> SprsError {
    SprsError::Internal {
        message: message.into(),
        location: None,
    }
}

fn is_handle_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Str
            | Type::Buffer
            | Type::AtomVal
            | Type::ClosedLabelSet(_)
            | Type::Range
            | Type::Atom(_)
    ) || matches!(
        ty,
        Type::App(name, _) if name == "List" || name == "Process" || name == "Label"
    )
}

fn is_user_struct(ty: &Type) -> bool {
    is_user_struct_type(ty)
}

/// Matches `Compiler::register_struct`: these fields are i64 in the
/// compatibility RuntimeValue slab, not a full `{tag,data}` slot.
pub(crate) fn slab_stores_i64_data(ty: &Type) -> bool {
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
            | Type::TypeUsize
            | Type::Bool
            | Type::Str
            | Type::Float
            | Type::TypeF16
            | Type::TypeF32
            | Type::TypeF64
    )
}

fn runtime_tag_for(ty: &Type) -> Result<Tag, SprsError> {
    match ty {
        Type::Int | Type::TypeI64 => Ok(Tag::Integer),
        Type::TypeI8 => Ok(Tag::Int8),
        Type::TypeU8 => Ok(Tag::Uint8),
        Type::TypeI16 => Ok(Tag::Int16),
        Type::TypeU16 => Ok(Tag::Uint16),
        Type::TypeI32 => Ok(Tag::Int32),
        Type::TypeU32 => Ok(Tag::Uint32),
        Type::TypeU64 | Type::TypeUsize => Ok(Tag::Uint64),
        Type::Float | Type::TypeF64 => Ok(Tag::Float),
        Type::TypeF32 => Ok(Tag::Float32),
        Type::TypeF16 => Ok(Tag::Float16),
        Type::Bool => Ok(Tag::Boolean),
        Type::Str => Ok(Tag::String),
        Type::Buffer => Ok(Tag::Buffer),
        Type::Range => Ok(Tag::Range),
        Type::App(name, _) if name == "Label" => Ok(Tag::Label),
        Type::AtomVal | Type::Atom(_) | Type::ClosedLabelSet(_) => Ok(Tag::Atom),
        Type::App(name, _) if name == "List" => Ok(Tag::List),
        Type::RawPtr => Ok(Tag::RawPtr),
        Type::App(name, _) if name == "Ptr" => Ok(Tag::RawPtr),
        Type::Unit => Ok(Tag::Unit),
        Type::App(name, _) if name == "Process" => {
            Err(storage_error("Process(T) has no runtime StorageRep tag"))
        }
        other if is_user_struct(other) => Ok(Tag::Struct),
        other if is_handle_type(other) => Err(storage_error(format!(
            "no runtime tag for StorageRep of {other}"
        ))),
        other => Err(storage_error(format!(
            "no runtime tag for StorageRep of {other}"
        ))),
    }
}

fn load_runtime_data<'ctx>(
    compiler: &Compiler<'ctx>,
    value_ptr: PointerValue<'ctx>,
    name: &str,
) -> IntValue<'ctx> {
    let data_ptr = compiler
        .builder
        .build_struct_gep(
            compiler.runtime_value_type,
            value_ptr,
            1,
            &format!("{name}_data_ptr"),
        )
        .unwrap();
    compiler
        .builder
        .build_load(
            compiler.context.i64_type(),
            data_ptr,
            &format!("{name}_data"),
        )
        .unwrap()
        .into_int_value()
}

fn wrap_runtime<'ctx>(
    compiler: &mut Compiler<'ctx>,
    tag: Tag,
    data: IntValue<'ctx>,
    name: &str,
) -> Result<PointerValue<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(compiler, name)?;
    compiler.build_runtime_value_store(ptr, StoreTag::Int(tag as u64), StoreValue::Int(data), name);
    Ok(ptr)
}

pub fn compile_storage_place<'ctx>(
    compiler: &mut Compiler<'ctx>,
    pointer: &hir::Expr,
    module: &Module<'ctx>,
) -> Result<PointerValue<'ctx>, SprsError> {
    let ptr_val = compiler.compile_expr(pointer, module)?.into_pointer_value();
    let data_ptr = compiler
        .builder
        .build_struct_gep(compiler.runtime_value_type, ptr_val, 1, "storage_addr_ptr")
        .unwrap();
    let addr = compiler
        .builder
        .build_load(compiler.context.i64_type(), data_ptr, "storage_addr")
        .unwrap()
        .into_int_value();
    let ptr_ty = compiler.context.ptr_type(AddressSpace::default());
    Ok(compiler
        .builder
        .build_int_to_ptr(addr, ptr_ty, "storage_place")
        .unwrap())
}

pub fn load_storage_as_runtime<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    place: PointerValue<'ctx>,
    ty: &Type,
    mode: StorageLoad,
) -> Result<PointerValue<'ctx>, SprsError> {
    let ty = unwrap_storage_type(ty);
    let loaded = load_storage_value(compiler, module, place, &ty)?;
    match mode {
        StorageLoad::Clone => {
            let cloned = clone_runtime_value(compiler, loaded, module)?;
            if is_user_struct(&ty) {
                let handle = load_runtime_data(compiler, loaded, "clone_temp_struct");
                let forget_fn = compiler.get_runtime_fn(module, "__struct_forget_owned")?;
                compiler
                    .builder
                    .build_call(forget_fn, &[handle.into()], "clone_forget_temp")
                    .unwrap();
                let drop_fn = compiler.get_runtime_fn(module, "__drop")?;
                drop_var(compiler, loaded, drop_fn, "clone_temp_struct");
            }
            Ok(cloned)
        }
        StorageLoad::Move => Ok(loaded),
    }
}

pub fn store_runtime_to_storage<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    place: PointerValue<'ctx>,
    ty: &Type,
    value_ptr: PointerValue<'ctx>,
    drop_old: bool,
) -> Result<(), SprsError> {
    let ty = unwrap_storage_type(ty);
    if drop_old {
        drop_storage(compiler, module, place, &ty)?;
    }
    store_storage_value(compiler, module, place, &ty, value_ptr)
}

pub fn drop_storage<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    place: PointerValue<'ctx>,
    ty: &Type,
) -> Result<(), SprsError> {
    let ty = unwrap_storage_type(ty);
    match &ty {
        Type::TypeI8
        | Type::TypeU8
        | Type::TypeI16
        | Type::TypeU16
        | Type::TypeI32
        | Type::TypeU32
        | Type::TypeI64
        | Type::TypeU64
        | Type::TypeUsize
        | Type::Int
        | Type::TypeF16
        | Type::TypeF32
        | Type::TypeF64
        | Type::Float
        | Type::Bool
        | Type::Unit
        | Type::RawPtr => Ok(()),
        Type::App(name, _) if name == "Ptr" || name == "Process" => Ok(()),
        other if is_user_struct(other) => {
            let fields = compiler.struct_storage_fields(other)?;
            let layout = compiler.storage_layout(other)?;
            let struct_ty = layout.llvm_type.into_struct_type();
            for (index, (_, field_ty)) in fields.iter().enumerate() {
                let field_ptr = compiler
                    .builder
                    .build_struct_gep(
                        struct_ty,
                        place,
                        index as u32,
                        &format!("drop_field_{index}"),
                    )
                    .unwrap();
                drop_storage(compiler, module, field_ptr, field_ty)?;
            }
            Ok(())
        }
        other => {
            let value = load_storage_value(compiler, module, place, other)?;
            let drop_fn = compiler.get_runtime_fn(module, "__drop")?;
            drop_var(compiler, value, drop_fn, "storage_drop");
            Ok(())
        }
    }
}

fn load_storage_value<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    place: PointerValue<'ctx>,
    ty: &Type,
) -> Result<PointerValue<'ctx>, SprsError> {
    if is_user_struct(ty) {
        return unpack_struct(compiler, module, place, ty);
    }
    if ptr_element(ty).is_some() || matches!(ty, Type::RawPtr) {
        let ptr = compiler
            .builder
            .build_load(
                compiler.context.ptr_type(AddressSpace::default()),
                place,
                "load_ptr",
            )
            .unwrap()
            .into_pointer_value();
        let addr = compiler
            .builder
            .build_ptr_to_int(ptr, compiler.context.i64_type(), "load_ptr_addr")
            .unwrap();
        return wrap_runtime(compiler, Tag::RawPtr, addr, "ptr_runtime");
    }
    if is_tagged_storage(ty) {
        let loaded = compiler
            .builder
            .build_load(compiler.runtime_value_type, place, "load_tagged")
            .unwrap();
        let ptr = create_entry_block_alloca(compiler, "tagged_runtime")?;
        compiler.builder.build_store(ptr, loaded).unwrap();
        return Ok(ptr);
    }
    if is_handle_type(ty) {
        let handle = compiler
            .builder
            .build_load(compiler.context.i64_type(), place, "load_handle")
            .unwrap()
            .into_int_value();
        return wrap_runtime(compiler, runtime_tag_for(ty)?, handle, "handle_runtime");
    }
    match ty {
        Type::Bool => {
            let byte = compiler
                .builder
                .build_load(compiler.context.i8_type(), place, "load_bool")
                .unwrap()
                .into_int_value();
            let data = compiler
                .builder
                .build_int_z_extend(byte, compiler.context.i64_type(), "bool_data")
                .unwrap();
            wrap_runtime(compiler, Tag::Boolean, data, "bool_runtime")
        }
        Type::TypeF32 => {
            let bits = load_float_bits(compiler, place, compiler.context.f32_type().into(), "f32")?;
            wrap_runtime(compiler, Tag::Float32, bits, "f32_runtime")
        }
        Type::Float | Type::TypeF64 => {
            let bits = load_float_bits(compiler, place, compiler.context.f64_type().into(), "f64")?;
            wrap_runtime(compiler, runtime_tag_for(ty)?, bits, "f64_runtime")
        }
        Type::Unit => wrap_runtime(
            compiler,
            Tag::Unit,
            compiler.context.i64_type().const_zero(),
            "unit_runtime",
        ),
        _ => {
            let layout = compiler.storage_layout(ty)?;
            let loaded = compiler
                .builder
                .build_load(layout.llvm_type, place, "load_scalar")
                .unwrap();
            let data = int_to_i64(compiler, loaded, ty)?;
            wrap_runtime(compiler, runtime_tag_for(ty)?, data, "scalar_runtime")
        }
    }
}

fn store_storage_value<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    place: PointerValue<'ctx>,
    ty: &Type,
    value_ptr: PointerValue<'ctx>,
) -> Result<(), SprsError> {
    if is_user_struct(ty) {
        return pack_struct(compiler, module, place, ty, value_ptr);
    }
    if ptr_element(ty).is_some() || matches!(ty, Type::RawPtr) {
        let addr = load_runtime_data(compiler, value_ptr, "store_ptr");
        let ptr = compiler
            .builder
            .build_int_to_ptr(
                addr,
                compiler.context.ptr_type(AddressSpace::default()),
                "store_ptr_val",
            )
            .unwrap();
        compiler.builder.build_store(place, ptr).unwrap();
        return Ok(());
    }
    if is_tagged_storage(ty) {
        let loaded = compiler
            .builder
            .build_load(compiler.runtime_value_type, value_ptr, "store_tagged")
            .unwrap();
        compiler.builder.build_store(place, loaded).unwrap();
        compiler.build_tag_store(
            Tag::Unit,
            compiler.build_tag_gep(value_ptr, "store_tagged_src"),
        );
        return Ok(());
    }
    if is_handle_type(ty) {
        let handle = load_runtime_data(compiler, value_ptr, "store_handle");
        compiler.builder.build_store(place, handle).unwrap();
        compiler.build_tag_store(
            Tag::Unit,
            compiler.build_tag_gep(value_ptr, "store_handle_src"),
        );
        return Ok(());
    }
    match ty {
        Type::Bool => {
            let data = load_runtime_data(compiler, value_ptr, "store_bool");
            let byte = compiler
                .builder
                .build_int_truncate(data, compiler.context.i8_type(), "bool_byte")
                .unwrap();
            compiler.builder.build_store(place, byte).unwrap();
        }
        Type::TypeF32 => store_float_bits(
            compiler,
            place,
            value_ptr,
            compiler.context.f32_type().into(),
            "f32",
        )?,
        Type::Float | Type::TypeF64 => store_float_bits(
            compiler,
            place,
            value_ptr,
            compiler.context.f64_type().into(),
            "f64",
        )?,
        Type::Unit => {}
        _ => {
            let layout = compiler.storage_layout(ty)?;
            let data = load_runtime_data(compiler, value_ptr, "store_scalar");
            let stored = i64_to_int(compiler, data, layout.llvm_type, ty)?;
            compiler.builder.build_store(place, stored).unwrap();
        }
    }
    Ok(())
}

fn load_float_bits<'ctx>(
    compiler: &Compiler<'ctx>,
    place: PointerValue<'ctx>,
    float_ty: BasicTypeEnum<'ctx>,
    name: &str,
) -> Result<IntValue<'ctx>, SprsError> {
    let loaded = compiler
        .builder
        .build_load(float_ty, place, &format!("load_{name}"))
        .unwrap();
    let bits_ty = match float_ty {
        BasicTypeEnum::FloatType(ty) if ty == compiler.context.f32_type() => {
            compiler.context.i32_type()
        }
        _ => compiler.context.i64_type(),
    };
    let bits = compiler
        .builder
        .build_bit_cast(loaded, bits_ty, &format!("{name}_bits"))
        .unwrap()
        .into_int_value();
    if bits.get_type() == compiler.context.i64_type() {
        Ok(bits)
    } else {
        Ok(compiler
            .builder
            .build_int_z_extend(bits, compiler.context.i64_type(), &format!("{name}_zext"))
            .unwrap())
    }
}

fn store_float_bits<'ctx>(
    compiler: &Compiler<'ctx>,
    place: PointerValue<'ctx>,
    value_ptr: PointerValue<'ctx>,
    float_ty: BasicTypeEnum<'ctx>,
    name: &str,
) -> Result<(), SprsError> {
    let data = load_runtime_data(compiler, value_ptr, name);
    let bits_ty = match float_ty {
        BasicTypeEnum::FloatType(ty) if ty == compiler.context.f32_type() => {
            compiler.context.i32_type()
        }
        _ => compiler.context.i64_type(),
    };
    let bits = if bits_ty == compiler.context.i64_type() {
        data
    } else {
        compiler
            .builder
            .build_int_truncate(data, bits_ty, &format!("{name}_trunc"))
            .unwrap()
    };
    let float_val = compiler
        .builder
        .build_bit_cast(bits, float_ty, &format!("{name}_float"))
        .unwrap();
    compiler.builder.build_store(place, float_val).unwrap();
    Ok(())
}

fn int_to_i64<'ctx>(
    compiler: &Compiler<'ctx>,
    loaded: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<IntValue<'ctx>, SprsError> {
    let value = loaded.into_int_value();
    let dest = compiler.context.i64_type();
    if value.get_type() == dest {
        return Ok(value);
    }
    let signed = matches!(
        ty,
        Type::TypeI8 | Type::TypeI16 | Type::TypeI32 | Type::TypeI64 | Type::Int
    );
    if signed {
        Ok(compiler
            .builder
            .build_int_s_extend(value, dest, "sext_i64")
            .unwrap())
    } else {
        Ok(compiler
            .builder
            .build_int_z_extend(value, dest, "zext_i64")
            .unwrap())
    }
}

fn i64_to_int<'ctx>(
    compiler: &Compiler<'ctx>,
    data: IntValue<'ctx>,
    llvm_type: BasicTypeEnum<'ctx>,
    _ty: &Type,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let BasicTypeEnum::IntType(int_ty) = llvm_type else {
        return Err(storage_error("expected integer StorageRep"));
    };
    if data.get_type() == int_ty {
        return Ok(data.into());
    }
    Ok(compiler
        .builder
        .build_int_truncate(data, int_ty, "trunc_store")
        .unwrap()
        .into())
}

fn unpack_struct<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    place: PointerValue<'ctx>,
    ty: &Type,
) -> Result<PointerValue<'ctx>, SprsError> {
    let backend = runtime_struct_backend_name(compiler, ty)?;
    let (slab_ty, field_tys) = {
        let def = compiler
            .struct_defs
            .get(&backend)
            .ok_or_else(|| storage_error(format!("undefined struct `{backend}`")))?;
        (
            def.llvm_type,
            def.fields
                .iter()
                .map(|field| field.ty.clone())
                .collect::<Vec<_>>(),
        )
    };
    let storage_layout = compiler.storage_layout(ty)?;
    let storage_ty = storage_layout.llvm_type.into_struct_type();
    let size_bytes = compiler
        .target_machine
        .get_target_data()
        .get_store_size(&slab_ty.as_any_type_enum());
    let size = compiler.context.i64_type().const_int(size_bytes, false);
    let new_fn = compiler.get_runtime_fn(module, "__struct_new")?;
    let handle = match compiler
        .builder
        .build_call(new_fn, &[size.into()], "storage_struct_new")
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => return Err(storage_error("__struct_new returned void")),
    };
    let borrow_fn = compiler.get_runtime_fn(module, "__struct_borrow")?;
    let heap_ptr = match compiler
        .builder
        .build_call(borrow_fn, &[handle.into()], "storage_struct_borrow")
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_pointer_value(),
        _ => return Err(storage_error("__struct_borrow returned void")),
    };
    let heap_ptr = compiler
        .builder
        .build_pointer_cast(
            heap_ptr,
            compiler.context.ptr_type(AddressSpace::default()),
            "unpack_heap",
        )
        .unwrap();
    let fields = compiler.struct_storage_fields(ty)?;
    for (index, (_, field_ty)) in fields.iter().enumerate() {
        let src = compiler
            .builder
            .build_struct_gep(
                storage_ty,
                place,
                index as u32,
                &format!("unpack_src_{index}"),
            )
            .unwrap();
        let field_runtime = load_storage_value(compiler, module, src, field_ty)?;
        let dest = compiler
            .builder
            .build_struct_gep(
                slab_ty,
                heap_ptr,
                index as u32,
                &format!("unpack_dest_{index}"),
            )
            .unwrap();
        write_runtime_struct_field(
            compiler,
            module,
            handle,
            dest,
            field_runtime,
            field_tys.get(index).and_then(|ty| ty.as_ref()),
        )?;
    }
    wrap_runtime(compiler, Tag::Struct, handle, "struct_runtime")
}

fn pack_struct<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    place: PointerValue<'ctx>,
    ty: &Type,
    value_ptr: PointerValue<'ctx>,
) -> Result<(), SprsError> {
    let backend = runtime_struct_backend_name(compiler, ty)?;
    let slab_ty = compiler
        .struct_defs
        .get(&backend)
        .ok_or_else(|| storage_error(format!("undefined struct `{backend}`")))?
        .llvm_type;
    let handle = load_runtime_data(compiler, value_ptr, "pack_struct");
    let borrow_fn = compiler.get_runtime_fn(module, "__struct_borrow")?;
    let heap_ptr = match compiler
        .builder
        .build_call(borrow_fn, &[handle.into()], "pack_struct_borrow")
        .unwrap()
        .try_as_basic_value()
    {
        ValueKind::Basic(val) => val.into_pointer_value(),
        _ => return Err(storage_error("__struct_borrow returned void")),
    };
    let heap_ptr = compiler
        .builder
        .build_pointer_cast(
            heap_ptr,
            compiler.context.ptr_type(AddressSpace::default()),
            "pack_heap",
        )
        .unwrap();
    let storage_layout = compiler.storage_layout(ty)?;
    let storage_ty = storage_layout.llvm_type.into_struct_type();
    let fields = compiler.struct_storage_fields(ty)?;
    let field_tys = compiler
        .struct_defs
        .get(&backend)
        .unwrap()
        .fields
        .iter()
        .map(|field| field.ty.clone())
        .collect::<Vec<_>>();
    for (index, (_, field_ty)) in fields.iter().enumerate() {
        let src = compiler
            .builder
            .build_struct_gep(
                slab_ty,
                heap_ptr,
                index as u32,
                &format!("pack_src_{index}"),
            )
            .unwrap();
        let field_runtime = read_runtime_struct_field(
            compiler,
            module,
            src,
            field_tys
                .get(index)
                .and_then(|ty| ty.as_ref())
                .unwrap_or(field_ty),
        )?;
        let dest = compiler
            .builder
            .build_struct_gep(
                storage_ty,
                place,
                index as u32,
                &format!("pack_dest_{index}"),
            )
            .unwrap();
        store_storage_value(compiler, module, dest, field_ty, field_runtime)?;
    }
    let forget_fn = compiler.get_runtime_fn(module, "__struct_forget_owned")?;
    compiler
        .builder
        .build_call(forget_fn, &[handle.into()], "pack_forget_owned")
        .unwrap();
    let drop_fn = compiler.get_runtime_fn(module, "__drop")?;
    drop_var(compiler, value_ptr, drop_fn, "pack_struct_drop");
    compiler.build_tag_store(
        Tag::Unit,
        compiler.build_tag_gep(value_ptr, "pack_struct_src"),
    );
    Ok(())
}

fn runtime_struct_backend_name(compiler: &Compiler, ty: &Type) -> Result<String, SprsError> {
    match ty {
        Type::Struct(name) => Ok(name.clone()),
        Type::App(name, args) => compiler
            .struct_specialization_names
            .iter()
            .find(|(id, _)| id.declaration.name == *name && id.args == *args)
            .map(|(_, backend)| backend.clone())
            .ok_or_else(|| storage_error(format!("missing specialization for {ty}"))),
        other => Err(storage_error(format!("not a struct: {other}"))),
    }
}

fn write_runtime_struct_field<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    handle: IntValue<'ctx>,
    field_ptr: PointerValue<'ctx>,
    value_ptr: PointerValue<'ctx>,
    field_ty: Option<&Type>,
) -> Result<(), SprsError> {
    if field_ty.is_some_and(slab_stores_i64_data) {
        let data = load_runtime_data(compiler, value_ptr, "field_imm");
        compiler.builder.build_store(field_ptr, data).unwrap();
        if matches!(field_ty, Some(Type::Str)) {
            let tag = compiler
                .context
                .i32_type()
                .const_int(Tag::String as u64, false);
            emit_track(compiler, module, handle, field_ptr, tag, data, true)?;
            compiler.build_tag_store(
                Tag::Unit,
                compiler.build_tag_gep(value_ptr, "field_str_src"),
            );
        }
        return Ok(());
    }
    let loaded = compiler
        .builder
        .build_load(compiler.runtime_value_type, value_ptr, "field_rv")
        .unwrap();
    compiler.builder.build_store(field_ptr, loaded).unwrap();
    let tag_ptr = compiler.build_tag_gep(value_ptr, "field_rv");
    let tag = compiler.build_load_tag(tag_ptr, "field_rv");
    let data = load_runtime_data(compiler, value_ptr, "field_rv");
    emit_track(compiler, module, handle, field_ptr, tag, data, false)?;
    compiler.build_tag_store(Tag::Unit, tag_ptr);
    Ok(())
}

fn read_runtime_struct_field<'ctx>(
    compiler: &mut Compiler<'ctx>,
    _module: &Module<'ctx>,
    field_ptr: PointerValue<'ctx>,
    field_ty: &Type,
) -> Result<PointerValue<'ctx>, SprsError> {
    if slab_stores_i64_data(field_ty) {
        let val = compiler
            .builder
            .build_load(compiler.context.i64_type(), field_ptr, "read_imm_field")
            .unwrap()
            .into_int_value();
        return wrap_runtime(compiler, runtime_tag_for(field_ty)?, val, "imm_field");
    }
    let ptr = create_entry_block_alloca(compiler, "read_field_rv")?;
    let loaded = compiler
        .builder
        .build_load(compiler.runtime_value_type, field_ptr, "read_generic_field")
        .unwrap();
    compiler.builder.build_store(ptr, loaded).unwrap();
    Ok(ptr)
}

fn emit_track<'ctx>(
    compiler: &mut Compiler<'ctx>,
    module: &Module<'ctx>,
    handle: IntValue<'ctx>,
    field_ptr: PointerValue<'ctx>,
    tag: IntValue<'ctx>,
    data: IntValue<'ctx>,
    data_only: bool,
) -> Result<(), SprsError> {
    let track_fn = compiler.get_runtime_fn(module, "__struct_track_value")?;
    let data_only_val = compiler
        .context
        .i32_type()
        .const_int(if data_only { 1 } else { 0 }, false);
    compiler
        .builder
        .build_call(
            track_fn,
            &[
                handle.into(),
                field_ptr.into(),
                tag.into(),
                data.into(),
                data_only_val.into(),
            ],
            "storage_track",
        )
        .unwrap();
    Ok(())
}

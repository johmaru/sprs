use crate::front::error::SprsError;
use crate::llvm::arithmetic::{is_float_family_tag, is_integer_family_tag, is_unsigned_int_tag};
use crate::llvm::value::{create_entry_block_alloca, create_error_label_from_str};
use crate::{
    front::hir,
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use inkwell::values::{BasicValueEnum, PointerValue, ValueKind};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};

pub enum UpDown {
    Up = 0,
    Down = 1,
}

pub fn create_increment_or_decrement<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    expr: &hir::Expr,
    mode: UpDown,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let val_ptr = self_compiler
        .compile_expr(expr, module)?
        .into_pointer_value();

    let mode_str = match mode {
        UpDown::Up => "increment",
        UpDown::Down => "decrement",
    };

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            0,
            format!("{}_tag_ptr", mode_str).as_str(),
        )
        .unwrap();
    let tag = self_compiler
        .builder
        .build_load(
            self_compiler.context.i32_type(),
            tag_ptr,
            format!("{}_tag", mode_str).as_str(),
        )
        .unwrap()
        .into_int_value();

    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let ok_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{mode_str}_ok"));
    let err_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{mode_str}_err"));
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{mode_str}_merge"));

    let is_int = is_integer_family_tag(self_compiler, tag)?;
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_int, ok_bb, err_bb);

    self_compiler.builder.position_at_end(err_bb);
    let error_ptr =
        create_error_label_from_str(self_compiler, "TypeError: type miss match", module)?;
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(ok_bb);
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            val_ptr,
            1,
            format!("{}_data_ptr", mode_str).as_str(),
        )
        .unwrap();
    let val = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            data_ptr,
            format!("{}_val", mode_str).as_str(),
        )
        .unwrap()
        .into_int_value();

    let one = self_compiler.context.i64_type().const_int(1, false);
    match mode {
        UpDown::Up => {
            let incremented = self_compiler
                .builder
                .build_int_add(val, one, "incremented")
                .unwrap();
            self_compiler
                .builder
                .build_store(data_ptr, incremented)
                .unwrap();
        }
        UpDown::Down => {
            let decremented = self_compiler
                .builder
                .build_int_sub(val, one, "decremented")
                .unwrap();
            self_compiler
                .builder
                .build_store(data_ptr, decremented)
                .unwrap();
        }
    }
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(merge_bb);
    let phi = self_compiler
        .builder
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            &format!("{mode_str}_res_phi"),
        )
        .unwrap();
    phi.add_incoming(&[(&val_ptr, ok_bb), (&error_ptr, err_bb)]);
    Ok(phi.as_basic_value())
}

pub enum EqNeq {
    Eq = 0,
    Neq = 1,
}

pub fn create_eq_or_neq<'ctx, ComparisonBuilder>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &hir::Expr,
    rhs: &hir::Expr,
    module: &inkwell::module::Module<'ctx>,
    mode: EqNeq,
    op_fn: ComparisonBuilder,
) -> Result<BasicValueEnum<'ctx>, SprsError>
where
    ComparisonBuilder: Fn(
        &inkwell::builder::Builder<'ctx>,
        inkwell::values::IntValue<'ctx>,
        inkwell::values::IntValue<'ctx>,
        &str,
    ) -> Result<inkwell::values::IntValue<'ctx>, SprsError>,
{
    let l_ptr = self_compiler
        .compile_expr(lhs, module)?
        .into_pointer_value();
    let r_ptr = self_compiler
        .compile_expr(rhs, module)?
        .into_pointer_value();

    let l_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 0, "l_tag_ptr")
        .unwrap();
    let l_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), l_tag_ptr, "l_tag")
        .unwrap()
        .into_int_value();

    let r_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 0, "r_tag_ptr")
        .unwrap();
    let r_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), r_tag_ptr, "r_tag")
        .unwrap()
        .into_int_value();

    let l_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), l_data_ptr, "l_val")
        .unwrap()
        .into_int_value();

    let r_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), r_data_ptr, "r_val")
        .unwrap()
        .into_int_value();

    let atom_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Atom as u64, false);
    let l_is_atom = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, atom_tag, "l_is_atom")
        .unwrap();
    let r_is_atom = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, r_tag, atom_tag, "r_is_atom")
        .unwrap();
    let either_atom = self_compiler
        .builder
        .build_or(l_is_atom, r_is_atom, "either_atom")
        .unwrap();

    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let atom_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "eq_atom");
    let check_str_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "eq_check_str");
    let str_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "eq_str");
    let check_float_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "eq_check_float");
    let float_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "eq_float");
    let data_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "eq_data");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "eq_merge");
    self_compiler
        .builder
        .build_conditional_branch(either_atom, atom_bb, check_str_bb)
        .unwrap();

    self_compiler.builder.position_at_end(atom_bb);
    let tag_eq = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, r_tag, "atom_tag_eq")
        .unwrap();
    let data_eq = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_val, r_val, "atom_data_eq")
        .unwrap();
    let atom_eq = self_compiler
        .builder
        .build_and(tag_eq, data_eq, "atom_eq")
        .unwrap();
    let atom_result = match mode {
        EqNeq::Eq => atom_eq,
        EqNeq::Neq => self_compiler
            .builder
            .build_xor(
                atom_eq,
                self_compiler.context.bool_type().const_int(1, false),
                "atom_neq",
            )
            .unwrap(),
    };
    self_compiler
        .builder
        .build_unconditional_branch(merge_bb)
        .unwrap();

    self_compiler.builder.position_at_end(check_str_bb);
    let string_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::String as u64, false);
    let l_is_str = self_compiler
        .builder
        .build_int_compare(IntPredicate::EQ, l_tag, string_tag, "l_is_str")
        .unwrap();
    let r_is_str = self_compiler
        .builder
        .build_int_compare(IntPredicate::EQ, r_tag, string_tag, "r_is_str")
        .unwrap();
    let both_str = self_compiler
        .builder
        .build_and(l_is_str, r_is_str, "both_str")
        .unwrap();
    let _ = self_compiler
        .builder
        .build_conditional_branch(both_str, str_bb, check_float_bb);

    self_compiler.builder.position_at_end(str_bb);
    let string_eq_fn = self_compiler.get_runtime_fn(module, "__string_eq")?;
    let eq_call = self_compiler
        .builder
        .build_call(
            string_eq_fn,
            &[l_val.into(), r_val.into()],
            "string_eq_call",
        )
        .unwrap();
    let eq_i32 = match eq_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => {
            return Err(SprsError::Internal {
                message: "Expected i32 from __string_eq".to_string(),
                location: None,
            });
        }
    };
    let eq_nonzero = self_compiler
        .builder
        .build_int_compare(
            IntPredicate::NE,
            eq_i32,
            self_compiler.context.i32_type().const_int(0, false),
            "string_eq_bool",
        )
        .unwrap();
    let str_result = match mode {
        EqNeq::Eq => eq_nonzero,
        EqNeq::Neq => self_compiler
            .builder
            .build_xor(
                eq_nonzero,
                self_compiler.context.bool_type().const_int(1, false),
                "string_neq",
            )
            .unwrap(),
    };
    let str_end_bb = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(check_float_bb);
    let l_float = is_float_family_tag(self_compiler, l_tag)?;
    let r_float = is_float_family_tag(self_compiler, r_tag)?;
    let both_float = self_compiler
        .builder
        .build_and(l_float, r_float, "both_float_eq")
        .unwrap();
    let _ = self_compiler
        .builder
        .build_conditional_branch(both_float, float_bb, data_bb);

    self_compiler.builder.position_at_end(float_bb);
    let l_f64 = bitcast_float_data_to_f64(self_compiler, l_val, l_tag);
    let r_f64 = bitcast_float_data_to_f64(self_compiler, r_val, r_tag);
    let pred = match mode {
        EqNeq::Eq => FloatPredicate::OEQ,
        EqNeq::Neq => FloatPredicate::ONE,
    };
    let float_result = self_compiler
        .builder
        .build_float_compare(pred, l_f64, r_f64, "float_eq")
        .unwrap();
    let float_end_bb = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(data_bb);
    let data_result = op_fn(
        &self_compiler.builder,
        l_val,
        r_val,
        match mode {
            EqNeq::Eq => "eq",
            EqNeq::Neq => "neq",
        },
    )?;
    self_compiler
        .builder
        .build_unconditional_branch(merge_bb)
        .unwrap();

    self_compiler.builder.position_at_end(merge_bb);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.context.bool_type(), "eq_phi")
        .unwrap();
    phi.add_incoming(&[
        (&atom_result, atom_bb),
        (&str_result, str_end_bb),
        (&float_result, float_end_bb),
        (&data_result, data_bb),
    ]);

    let res_ptr = create_entry_block_alloca(self_compiler, "eq_or_neq_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Bool(phi.as_basic_value().into_int_value()),
        "eq_or_neq_res",
    );

    Ok(res_ptr.into())
}

/// Interpret `data` bits as a float of `tag`'s width and extend to f64.
/// Mixed-width operands use each side's own tag (f16/f32 truncated then extended).
fn bitcast_float_data_to_f64<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    data: inkwell::values::IntValue<'ctx>,
    tag: inkwell::values::IntValue<'ctx>,
) -> inkwell::values::FloatValue<'ctx> {
    let parent = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let bb_f16 = self_compiler
        .context
        .append_basic_block(parent, "eq_as_f16");
    let bb_f32 = self_compiler
        .context
        .append_basic_block(parent, "eq_as_f32");
    let bb_f64 = self_compiler
        .context
        .append_basic_block(parent, "eq_as_f64");
    let merge = self_compiler
        .context
        .append_basic_block(parent, "eq_as_f64_merge");

    let f16_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float16 as u64, false);
    let f32_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float32 as u64, false);
    let cases = vec![(f16_tag, bb_f16), (f32_tag, bb_f32)];
    self_compiler
        .builder
        .build_switch(tag, bb_f64, &cases)
        .unwrap();

    self_compiler.builder.position_at_end(bb_f16);
    let i16 = self_compiler
        .builder
        .build_int_truncate(data, self_compiler.context.i16_type(), "eq_f16_trunc")
        .unwrap();
    let f16 = self_compiler
        .builder
        .build_bit_cast(i16, self_compiler.context.f16_type(), "eq_f16_bits")
        .unwrap()
        .into_float_value();
    let f16_ext = self_compiler
        .builder
        .build_float_ext(f16, self_compiler.context.f64_type(), "eq_f16_ext")
        .unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge);

    self_compiler.builder.position_at_end(bb_f32);
    let i32v = self_compiler
        .builder
        .build_int_truncate(data, self_compiler.context.i32_type(), "eq_f32_trunc")
        .unwrap();
    let f32 = self_compiler
        .builder
        .build_bit_cast(i32v, self_compiler.context.f32_type(), "eq_f32_bits")
        .unwrap()
        .into_float_value();
    let f32_ext = self_compiler
        .builder
        .build_float_ext(f32, self_compiler.context.f64_type(), "eq_f32_ext")
        .unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge);

    self_compiler.builder.position_at_end(bb_f64);
    let f64 = self_compiler
        .builder
        .build_bit_cast(data, self_compiler.context.f64_type(), "eq_f64_bits")
        .unwrap()
        .into_float_value();
    let _ = self_compiler.builder.build_unconditional_branch(merge);

    self_compiler.builder.position_at_end(merge);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.context.f64_type(), "eq_f64_phi")
        .unwrap();
    phi.add_incoming(&[(&f16_ext, bb_f16), (&f32_ext, bb_f32), (&f64, bb_f64)]);
    phi.as_basic_value().into_float_value()
}

pub enum Comparison {
    Gt = 0,
    Lt = 1,
    Ge = 2,
    Le = 3,
}

pub fn create_comparison<'ctx, ComparisonBuilder>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &hir::Expr,
    rhs: &hir::Expr,
    module: &inkwell::module::Module<'ctx>,
    mode: Comparison,
    _comp_fn: ComparisonBuilder,
) -> Result<BasicValueEnum<'ctx>, SprsError>
where
    ComparisonBuilder: Fn(
        &inkwell::builder::Builder<'ctx>,
        inkwell::values::IntValue<'ctx>,
        inkwell::values::IntValue<'ctx>,
        &str,
    ) -> Result<inkwell::values::IntValue<'ctx>, SprsError>,
{
    let l_ptr = self_compiler
        .compile_expr(lhs, module)?
        .into_pointer_value();
    let r_ptr = self_compiler
        .compile_expr(rhs, module)?
        .into_pointer_value();

    let l_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 0, "l_tag_ptr")
        .unwrap();
    let l_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), l_tag_ptr, "l_tag")
        .unwrap()
        .into_int_value();
    let r_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 0, "r_tag_ptr")
        .unwrap();
    let r_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), r_tag_ptr, "r_tag")
        .unwrap()
        .into_int_value();

    let l_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), l_data_ptr, "l_val")
        .unwrap()
        .into_int_value();

    let r_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), r_data_ptr, "r_val")
        .unwrap()
        .into_int_value();

    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let float_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_float");
    let check_int_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_check_int");
    let int_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_int");
    let err_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_err");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_merge");

    let l_float = is_float_family_tag(self_compiler, l_tag)?;
    let r_float = is_float_family_tag(self_compiler, r_tag)?;
    let both_float = self_compiler
        .builder
        .build_and(l_float, r_float, "both_float_cmp")
        .unwrap();
    let _ = self_compiler
        .builder
        .build_conditional_branch(both_float, float_bb, check_int_bb);

    self_compiler.builder.position_at_end(check_int_bb);
    let l_int = is_integer_family_tag(self_compiler, l_tag)?;
    let r_int = is_integer_family_tag(self_compiler, r_tag)?;
    let both_int = self_compiler
        .builder
        .build_and(l_int, r_int, "both_int_cmp")
        .unwrap();
    let _ = self_compiler
        .builder
        .build_conditional_branch(both_int, int_bb, err_bb);

    self_compiler.builder.position_at_end(err_bb);
    let error_ptr =
        create_error_label_from_str(self_compiler, "TypeError: type miss match", module)?;
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(float_bb);
    let l_f64 = bitcast_float_data_to_f64(self_compiler, l_val, l_tag);
    let r_f64 = bitcast_float_data_to_f64(self_compiler, r_val, r_tag);
    let fpred = match mode {
        Comparison::Lt => FloatPredicate::OLT,
        Comparison::Gt => FloatPredicate::OGT,
        Comparison::Le => FloatPredicate::OLE,
        Comparison::Ge => FloatPredicate::OGE,
    };
    let float_cmp = self_compiler
        .builder
        .build_float_compare(fpred, l_f64, r_f64, "float_cmp")
        .unwrap();
    let float_res = store_bool(self_compiler, float_cmp)?;
    let float_end = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(int_bb);
    let is_unsigned = is_unsigned_int_tag(self_compiler, l_tag)?;
    let u_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_uint");
    let s_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_sint");
    let int_merge = self_compiler
        .context
        .append_basic_block(parent_fn, "cmp_int_merge");
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_unsigned, u_bb, s_bb);

    self_compiler.builder.position_at_end(u_bb);
    let upred = match mode {
        Comparison::Lt => IntPredicate::ULT,
        Comparison::Gt => IntPredicate::UGT,
        Comparison::Le => IntPredicate::ULE,
        Comparison::Ge => IntPredicate::UGE,
    };
    let ucmp = self_compiler
        .builder
        .build_int_compare(upred, l_val, r_val, "ucmp")
        .unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(int_merge);

    self_compiler.builder.position_at_end(s_bb);
    let spred = match mode {
        Comparison::Lt => IntPredicate::SLT,
        Comparison::Gt => IntPredicate::SGT,
        Comparison::Le => IntPredicate::SLE,
        Comparison::Ge => IntPredicate::SGE,
    };
    let scmp = self_compiler
        .builder
        .build_int_compare(spred, l_val, r_val, "scmp")
        .unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(int_merge);

    self_compiler.builder.position_at_end(int_merge);
    let icmp_phi = self_compiler
        .builder
        .build_phi(self_compiler.context.bool_type(), "icmp_phi")
        .unwrap();
    icmp_phi.add_incoming(&[(&ucmp, u_bb), (&scmp, s_bb)]);
    let int_res = store_bool(self_compiler, icmp_phi.as_basic_value().into_int_value())?;
    let int_end = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(merge_bb);
    let phi = self_compiler
        .builder
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            "cmp_res_phi",
        )
        .unwrap();
    phi.add_incoming(&[
        (&float_res, float_end),
        (&int_res, int_end),
        (&error_ptr, err_bb),
    ]);
    Ok(phi.as_basic_value())
}

fn store_bool<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    value: inkwell::values::IntValue<'ctx>,
) -> Result<PointerValue<'ctx>, SprsError> {
    let res_ptr = create_entry_block_alloca(self_compiler, "comparison_res_alloc")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Bool(value),
        "comparison_res",
    );
    Ok(res_ptr)
}

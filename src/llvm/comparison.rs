use crate::front::error::SprsError;
use inkwell::values::{BasicValueEnum, PointerValue};
use crate::{
    front::ast,
    front::span::Spanned,
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use crate::llvm::value::create_entry_block_alloca;

pub enum UpDown {
    Up = 0,
    Down = 1,
}

pub fn create_increment_or_decrement<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    expr: &Spanned<ast::Expr>,
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

    Ok(val_ptr.into())
}

pub enum EqNeq {
    Eq = 0,
    Neq = 1,
}

pub fn create_eq_or_neq<'ctx, ComparisonBuilder>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
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

    // Atoms compare by tag AND interned id; other values keep the old
    // data-only comparison. Merge the two paths with a PHI.
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let atom_bb = self_compiler.context.append_basic_block(parent_fn, "eq_atom");
    let data_bb = self_compiler.context.append_basic_block(parent_fn, "eq_data");
    let merge_bb = self_compiler.context.append_basic_block(parent_fn, "eq_merge");
    self_compiler
        .builder
        .build_conditional_branch(either_atom, atom_bb, data_bb)
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
    self_compiler.builder.build_unconditional_branch(merge_bb).unwrap();

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
    self_compiler.builder.build_unconditional_branch(merge_bb).unwrap();

    self_compiler.builder.position_at_end(merge_bb);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.context.bool_type(), "eq_phi")
        .unwrap();
    phi.add_incoming(&[(&atom_result, atom_bb), (&data_result, data_bb)]);

    let res_ptr = create_entry_block_alloca(self_compiler, "eq_or_neq_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Bool(phi.as_basic_value().into_int_value()),
        "eq_or_neq_res",
    );

    Ok(res_ptr.into())
}

pub enum Comparison {
    Gt = 0,
    Lt = 1,
    Ge = 2,
    Le = 3,
}

pub fn create_comparison<'ctx, ComparisonBuilder>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
    mode: Comparison,
    comp_fn: ComparisonBuilder,
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

    let result = comp_fn(
        &self_compiler.builder,
        l_val,
        r_val,
        match mode {
            Comparison::Gt => "gt",
            Comparison::Lt => "lt",
            Comparison::Ge => "ge",
            Comparison::Le => "le",
        },
    )?;

    let res_ptr = create_entry_block_alloca(self_compiler, "comparison_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Bool(result),
        "comparison_res",
    );
    Ok(res_ptr.into())
}

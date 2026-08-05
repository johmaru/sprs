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

    let result = op_fn(
        &self_compiler.builder,
        l_val,
        r_val,
        match mode {
            EqNeq::Eq => "eq",
            EqNeq::Neq => "neq",
        },
    )?;

    let res_ptr = create_entry_block_alloca(self_compiler, "eq_or_neq_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Bool(result),
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

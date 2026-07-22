use inkwell::{
    values::{BasicValueEnum, PointerValue},
    builder::Builder,
};
use crate::{
    front::ast,
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use crate::llvm::value::{create_entry_block_alloca, create_panic_err, PanicErrorSettings};

pub fn create_if_condition<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    cond: &ast::Expr,
    then_blk: &Vec<ast::Stmt>,
    else_blk: &Option<Vec<ast::Stmt>>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<(), Box<dyn std::error::Error>> {
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let then_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "then_bb");
    let else_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "else_bb");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "if_merge");

    let cond_val = self_compiler.compile_expr(cond, module)?;
    let cond_ptr = cond_val.into_pointer_value();
    let cond_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            cond_ptr,
            1,
            "cond_data_ptr",
        )
        .unwrap();
    let cond_loaded = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            cond_data_ptr,
            "cond_loaded",
        )
        .unwrap()
        .into_int_value();
    let zero = self_compiler.context.i64_type().const_int(0, false);
    let cond_bool = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::NE, cond_loaded, zero, "if_cond_bool")
        .unwrap();

    let _ = self_compiler
        .builder
        .build_conditional_branch(cond_bool, then_bb, else_bb);

    self_compiler.builder.position_at_end(then_bb);
    self_compiler.compile_block(then_blk, module)?;
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }

    self_compiler.builder.position_at_end(else_bb);
    if let Some(else_blk) = else_blk {
        self_compiler.compile_block(else_blk, module)?;
    }
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }

    self_compiler.builder.position_at_end(merge_bb);
    Ok(())
}

pub fn create_while_condition<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    cond: &ast::Expr,
    body: &Vec<ast::Stmt>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<(), Box<dyn std::error::Error>> {
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let cond_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "while_cond");
    let body_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "while_body");
    let after_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "while_after");

    let _ = self_compiler.builder.build_unconditional_branch(cond_bb);
    self_compiler.builder.position_at_end(cond_bb);
    let cond_val = self_compiler.compile_expr(cond, module)?;
    let cond_ptr = cond_val.into_pointer_value();

    let cond_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            cond_ptr,
            1,
            "cond_data_ptr",
        )
        .unwrap();
    let cond_loaded = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            cond_data_ptr,
            "cond_loaded",
        )
        .unwrap()
        .into_int_value();

    let zero = self_compiler.context.i64_type().const_int(0, false);
    let cond_bool = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::NE,
            cond_loaded,
            zero,
            "while_cond_bool",
        )
        .unwrap();

    let _ = self_compiler
        .builder
        .build_conditional_branch(cond_bool, body_bb, after_bb);

    self_compiler.builder.position_at_end(body_bb);
    self_compiler.compile_block(body, module)?;

    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(cond_bb);
    }

    self_compiler.builder.position_at_end(after_bb);
    Ok(())
}

pub fn create_if_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    cond: &ast::Expr,
    then_expr: &ast::Expr,
    else_expr: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let then_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "then_bb");
    let else_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "else_bb");
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "if_merge");

    let cond_val = self_compiler.compile_expr(cond, module)?;
    let cond_ptr = cond_val.into_pointer_value();
    let cond_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            cond_ptr,
            1,
            "cond_data_ptr",
        )
        .unwrap();
    let cond_loaded = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            cond_data_ptr,
            "cond_loaded",
        )
        .unwrap()
        .into_int_value();
    let zero = self_compiler.context.i64_type().const_int(0, false);
    let cond_bool = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::NE, cond_loaded, zero, "if_cond_bool")
        .unwrap();

    let _ = self_compiler
        .builder
        .build_conditional_branch(cond_bool, then_bb, else_bb);

    self_compiler.builder.position_at_end(then_bb);
    let then_val = self_compiler.compile_expr(then_expr, module)?;
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }
    let then_bb_end = self_compiler.builder.get_insert_block().unwrap();

    // TODO: Handle case where else_expr, such as if (test) : ok() ? no();
    // TODO: Also  such as if (test) ok() orelse no();

    self_compiler.builder.position_at_end(else_bb);
    let else_val = self_compiler.compile_expr(else_expr, module)?;
    if self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_terminator()
        .is_none()
    {
        let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    }
    let else_bb_end = self_compiler.builder.get_insert_block().unwrap();

    self_compiler.builder.position_at_end(merge_bb);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.runtime_value_type, "if_phi")
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

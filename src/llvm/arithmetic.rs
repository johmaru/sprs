use crate::front::error::{SprsError, ErrorCode, ErrorCategory, Location};
use crate::front::span::Span;
use inkwell::{
    AddressSpace,
    values::{BasicValueEnum, IntValue, PointerValue, ValueKind},
};
use crate::{
    front::ast,
    front::span::Spanned,
    front::type_helper,
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use crate::llvm::value::{
    build_label_is_error, create_entry_block_alloca, create_error_label_from_str,
};
use crate::llvm::variable::move_variable;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl BinOpKind {
    fn name(self) -> &'static str {
        match self {
            BinOpKind::Add => "add",
            BinOpKind::Sub => "sub",
            BinOpKind::Mul => "mul",
            BinOpKind::Div => "div",
            BinOpKind::Mod => "mod",
        }
    }
}

pub fn create_add_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_binary_dispatch(self_compiler, lhs, rhs, module, BinOpKind::Add)
}

fn create_binary_dispatch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
    op: BinOpKind,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if op == BinOpKind::Add {
        if let Ok(val) = create_add_expr_type_check(self_compiler, lhs, rhs, module) {
            return Ok(val);
        }
    }

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

    let op_name = op.name();
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let int_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_int_bb"));
    let check_float_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_check_float_bb"));
    let float_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_float_bb"));
    let check_string_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_check_string_bb"));
    let string_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_string_bb"));
    let error_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_error_bb"));
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_merge_bb"));

    // short-circuit: if either operand is an error label, return it directly.
    let l_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), l_data_ptr, "l_data")
        .unwrap()
        .into_int_value();
    let l_is_error = build_label_is_error(self_compiler, l_tag, l_data, module)?;
    let l_error_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "l_error_short_circuit");
    let check_r_error_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "check_r_error");
    let _ = self_compiler.builder.build_conditional_branch(
        l_is_error,
        l_error_bb,
        check_r_error_bb,
    );

    self_compiler.builder.position_at_end(l_error_bb);
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(check_r_error_bb);
    let r_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), r_data_ptr, "r_data")
        .unwrap()
        .into_int_value();
    let r_is_error = build_label_is_error(self_compiler, r_tag, r_data, module)?;
    let r_error_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "r_error_short_circuit");
    let normal_dispatch_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "normal_dispatch");
    let _ = self_compiler.builder.build_conditional_branch(
        r_is_error,
        r_error_bb,
        normal_dispatch_bb,
    );

    self_compiler.builder.position_at_end(r_error_bb);
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(normal_dispatch_bb);

    // check if both are integers
    let can_int = create_add_expr_check_int(self_compiler, l_tag, r_tag)?;

    // check if both are float (Mod is not defined on floats)
    let both_float = if op == BinOpKind::Mod {
        self_compiler.context.bool_type().const_int(0, false)
    } else {
        create_add_expr_check_float(self_compiler, l_tag, r_tag)?
    };

    // check if both are strings (concatenation is Add-only)
    let check_string = if op == BinOpKind::Add {
        create_add_expr_check_string(self_compiler, l_tag, r_tag)?
    } else {
        self_compiler.context.bool_type().const_int(0, false)
    };

    let _ = self_compiler
        .builder
        .build_conditional_branch(can_int, int_bb, check_float_bb);

    // second check if can add as floats
    self_compiler.builder.position_at_end(check_float_bb);
    let _ = self_compiler
        .builder
        .build_conditional_branch(both_float, float_bb, check_string_bb);

    // third check if can add as strings
    self_compiler.builder.position_at_end(check_string_bb);
    let _ = self_compiler
        .builder
        .build_conditional_branch(check_string, string_bb, error_bb);

    // error branch
    self_compiler.builder.position_at_end(error_bb);

    let error_message = format!(
        "TypeError: type miss match : '{:?}' and '{:?}'",
        self_compiler.infer_type(lhs),
        self_compiler.infer_type(rhs)
    );

    let error_ptr = create_error_label_from_str(self_compiler, &error_message, module)?;

    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    // integer addition branch

    self_compiler.builder.position_at_end(int_bb);

    let int_res_ptr = create_add_expr_build_int_branch(
        self_compiler,
        module,
        l_ptr,
        r_ptr,
        l_tag,
        r_tag,
        op,
    )?;
    let int_end_bb = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    // float arithmetic branch

    self_compiler.builder.position_at_end(float_bb);

    let tags_equal = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, r_tag, "float_tags_equal")
        .unwrap();
    let float_default_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float as u64, false);
    let float_result_tag = self_compiler
        .builder
        .build_select(tags_equal, l_tag, float_default_tag, "float_result_tag")
        .unwrap()
        .into_int_value();

    let float_res_ptr = create_add_expr_build_float_branch(
        self_compiler,
        module,
        l_ptr,
        r_ptr,
        float_result_tag,
        op,
    )?;
    let float_end_bb = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
    // string concatenation branch

    self_compiler.builder.position_at_end(string_bb);

    let str_res_ptr = create_add_expr_build_string_branch(self_compiler, l_ptr, r_ptr, module)?;

    // final merge branch

    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(merge_bb);

    let phi = self_compiler
        .builder
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            "add_res_phi",
        )
        .unwrap();
    phi.add_incoming(&[
        (&int_res_ptr, int_end_bb),
        (&float_res_ptr, float_end_bb),
        (&str_res_ptr, string_bb),
        (&error_ptr, error_bb),
        (&l_ptr, l_error_bb),
        (&r_ptr, r_error_bb),
    ]);

    Ok(phi.as_basic_value())
}

fn create_add_expr_type_check<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let is_type = |expr: &Spanned<ast::Expr>, ty: &str| -> bool {
        match self_compiler.get_known_type_from_expr(expr) {
            Ok(known_type) => known_type == ty,
            Err(_) => false,
        }
    };

    if is_type(lhs, "i8") && is_type(rhs, "i8") {
        return create_int8_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "u8") && is_type(rhs, "u8") {
        return create_uint8_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "i16") && is_type(rhs, "i16") {
        return create_int16_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "u16") && is_type(rhs, "u16") {
        return create_uint16_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "i32") && is_type(rhs, "i32") {
        return create_int32_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "u32") && is_type(rhs, "u32") {
        return create_uint32_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "i64") && is_type(rhs, "i64") {
        return create_int64_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "u64") && is_type(rhs, "u64") {
        return create_uint64_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "f16") && is_type(rhs, "f16") {
        return create_float16_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "f32") && is_type(rhs, "f32") {
        return create_float32_add_logic(self_compiler, lhs, rhs, module);
    }

    if is_type(lhs, "f64") && is_type(rhs, "f64") {
        return create_float64_add_logic(self_compiler, lhs, rhs, module);
    }

    Err(SprsError::Semantic { code: ErrorCode { category: ErrorCategory::Semantic, number: 14 }, location: Location::new(String::new(), Span::DUMMY), message: "Unsupported types for addition".to_string(), help: None })
}

fn tag_eq_const<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    tag: IntValue<'ctx>,
    kind: Tag,
    name: &str,
) -> IntValue<'ctx> {
    let expected = self_compiler
        .context
        .i32_type()
        .const_int(kind as u64, false);
    self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, tag, expected, name)
        .unwrap()
}

fn or_tags<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    a: IntValue<'ctx>,
    b: IntValue<'ctx>,
    name: &str,
) -> IntValue<'ctx> {
    self_compiler.builder.build_or(a, b, name).unwrap()
}

pub(crate) fn is_integer_family_tag<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let is_int = tag_eq_const(self_compiler, tag, Tag::Integer, "is_integer");
    let is_i8 = tag_eq_const(self_compiler, tag, Tag::Int8, "is_int8");
    let is_u8 = tag_eq_const(self_compiler, tag, Tag::Uint8, "is_uint8");
    let is_i16 = tag_eq_const(self_compiler, tag, Tag::Int16, "is_int16");
    let is_u16 = tag_eq_const(self_compiler, tag, Tag::Uint16, "is_uint16");
    let is_i32 = tag_eq_const(self_compiler, tag, Tag::Int32, "is_int32");
    let is_u32 = tag_eq_const(self_compiler, tag, Tag::Uint32, "is_uint32");
    let is_i64 = tag_eq_const(self_compiler, tag, Tag::Int64, "is_int64");
    let is_u64 = tag_eq_const(self_compiler, tag, Tag::Uint64, "is_uint64");
    let a = or_tags(self_compiler, is_int, is_i8, "int_fam_0");
    let b = or_tags(self_compiler, a, is_u8, "int_fam_1");
    let c = or_tags(self_compiler, b, is_i16, "int_fam_2");
    let d = or_tags(self_compiler, c, is_u16, "int_fam_3");
    let e = or_tags(self_compiler, d, is_i32, "int_fam_4");
    let f = or_tags(self_compiler, e, is_u32, "int_fam_5");
    let g = or_tags(self_compiler, f, is_i64, "int_fam_6");
    Ok(or_tags(self_compiler, g, is_u64, "int_fam_final"))
}

pub(crate) fn is_unsigned_int_tag<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let is_u8 = tag_eq_const(self_compiler, tag, Tag::Uint8, "is_u8");
    let is_u16 = tag_eq_const(self_compiler, tag, Tag::Uint16, "is_u16");
    let is_u32 = tag_eq_const(self_compiler, tag, Tag::Uint32, "is_u32");
    let is_u64 = tag_eq_const(self_compiler, tag, Tag::Uint64, "is_u64");
    let a = or_tags(self_compiler, is_u8, is_u16, "uint_fam_0");
    let b = or_tags(self_compiler, a, is_u32, "uint_fam_1");
    Ok(or_tags(self_compiler, b, is_u64, "uint_fam_final"))
}

pub(crate) fn is_float_family_tag<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let is_f = tag_eq_const(self_compiler, tag, Tag::Float, "is_float");
    let is_f16 = tag_eq_const(self_compiler, tag, Tag::Float16, "is_float16");
    let is_f32 = tag_eq_const(self_compiler, tag, Tag::Float32, "is_float32");
    let is_f64 = tag_eq_const(self_compiler, tag, Tag::Float64, "is_float64");
    let a = or_tags(self_compiler, is_f, is_f16, "float_fam_0");
    let b = or_tags(self_compiler, a, is_f32, "float_fam_1");
    Ok(or_tags(self_compiler, b, is_f64, "float_fam_final"))
}

fn create_add_expr_check_int<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_tag: IntValue<'ctx>,
    r_tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let l_int = is_integer_family_tag(self_compiler, l_tag)?;
    let r_int = is_integer_family_tag(self_compiler, r_tag)?;
    Ok(self_compiler
        .builder
        .build_and(l_int, r_int, "both_int")
        .unwrap())
}

fn create_add_expr_check_string<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_tag: IntValue<'ctx>,
    r_tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let string_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::String as u64, false);
    let is_l_string = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, string_tag, "is_l_string")
        .unwrap();
    let is_r_string = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, r_tag, string_tag, "is_r_string")
        .unwrap();

    let both_string = self_compiler
        .builder
        .build_and(is_l_string, is_r_string, "both_string")
        .unwrap();

    Ok(both_string)
}

fn create_add_expr_check_float<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_tag: IntValue<'ctx>,
    r_tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, SprsError> {
    let l_float = is_float_family_tag(self_compiler, l_tag)?;
    let r_float = is_float_family_tag(self_compiler, r_tag)?;
    Ok(self_compiler
        .builder
        .build_and(l_float, r_float, "both_float")
        .unwrap())
}

fn create_add_expr_build_int_branch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    module: &inkwell::module::Module<'ctx>,
    l_ptr: PointerValue<'ctx>,
    r_ptr: PointerValue<'ctx>,
    l_tag: IntValue<'ctx>,
    r_tag: IntValue<'ctx>,
    op: BinOpKind,
) -> Result<PointerValue<'ctx>, SprsError> {
    let l_int_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_int_data_ptr")
        .unwrap();
    let l_int_val = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            l_int_data_ptr,
            "l_int_val",
        )
        .unwrap()
        .into_int_value();

    let r_int_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_int_data_ptr")
        .unwrap();
    let r_int_val = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            r_int_data_ptr,
            "r_int_val",
        )
        .unwrap()
        .into_int_value();

    let tags_equal = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, r_tag, "int_tags_equal")
        .unwrap();
    let integer_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Integer as u64, false);
    let result_tag = self_compiler
        .builder
        .build_select(tags_equal, l_tag, integer_tag, "int_result_tag")
        .unwrap()
        .into_int_value();

    if matches!(op, BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul) {
        let int_sum = match op {
            BinOpKind::Add => self_compiler
                .builder
                .build_int_add(l_int_val, r_int_val, "int_sum")
                .unwrap(),
            BinOpKind::Sub => self_compiler
                .builder
                .build_int_sub(l_int_val, r_int_val, "int_diff")
                .unwrap(),
            BinOpKind::Mul => self_compiler
                .builder
                .build_int_mul(l_int_val, r_int_val, "int_prod")
                .unwrap(),
            _ => unreachable!(),
        };
        let int_res_ptr = create_entry_block_alloca(self_compiler, "int_res_alloc")?;
        self_compiler.build_runtime_value_store(
            int_res_ptr,
            StoreTag::Dynamic(result_tag),
            StoreValue::Int(int_sum),
            "int_res",
        );
        return Ok(int_res_ptr);
    }

    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let op_name = op.name();
    let bb_ok = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_int_ok"));
    let bb_zero = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_int_zero"));
    let bb_unsigned = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_int_unsigned"));
    let bb_signed = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_int_signed"));
    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, &format!("{op_name}_int_merge"));

    let zero = self_compiler.context.i64_type().const_int(0, false);
    let is_zero = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, r_int_val, zero, "is_zero")
        .unwrap();
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_zero, bb_zero, bb_ok);

    self_compiler.builder.position_at_end(bb_zero);
    let error_ptr = create_error_label_from_str(self_compiler, "Division by zero", module)?;
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(bb_ok);
    let is_unsigned = is_unsigned_int_tag(self_compiler, l_tag)?;
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_unsigned, bb_unsigned, bb_signed);

    self_compiler.builder.position_at_end(bb_unsigned);
    let unsigned_res = match op {
        BinOpKind::Div => self_compiler
            .builder
            .build_int_unsigned_div(l_int_val, r_int_val, "udiv")
            .unwrap(),
        _ => self_compiler
            .builder
            .build_int_unsigned_rem(l_int_val, r_int_val, "urem")
            .unwrap(),
    };
    let unsigned_ptr = create_entry_block_alloca(self_compiler, "int_udiv_res")?;
    self_compiler.build_runtime_value_store(
        unsigned_ptr,
        StoreTag::Dynamic(result_tag),
        StoreValue::Int(unsigned_res),
        "int_udiv_res",
    );
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(bb_signed);
    let signed_res = match op {
        BinOpKind::Div => self_compiler
            .builder
            .build_int_signed_div(l_int_val, r_int_val, "sdiv")
            .unwrap(),
        _ => self_compiler
            .builder
            .build_int_signed_rem(l_int_val, r_int_val, "srem")
            .unwrap(),
    };
    let signed_ptr = create_entry_block_alloca(self_compiler, "int_sdiv_res")?;
    self_compiler.build_runtime_value_store(
        signed_ptr,
        StoreTag::Dynamic(result_tag),
        StoreValue::Int(signed_res),
        "int_sdiv_res",
    );
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(merge_bb);
    let phi = self_compiler
        .builder
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            "int_div_res_phi",
        )
        .unwrap();
    phi.add_incoming(&[
        (&error_ptr, bb_zero),
        (&unsigned_ptr, bb_unsigned),
        (&signed_ptr, bb_signed),
    ]);
    Ok(phi.as_basic_value().into_pointer_value())
}

fn apply_float_op<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    op: BinOpKind,
    lhs: inkwell::values::FloatValue<'ctx>,
    rhs: inkwell::values::FloatValue<'ctx>,
    name: &str,
) -> inkwell::values::FloatValue<'ctx> {
    match op {
        BinOpKind::Add => self_compiler.builder.build_float_add(lhs, rhs, name).unwrap(),
        BinOpKind::Sub => self_compiler.builder.build_float_sub(lhs, rhs, name).unwrap(),
        BinOpKind::Mul => self_compiler.builder.build_float_mul(lhs, rhs, name).unwrap(),
        BinOpKind::Div | BinOpKind::Mod => {
            self_compiler.builder.build_float_div(lhs, rhs, name).unwrap()
        }
    }
}

fn maybe_guard_float_div_zero<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    op: BinOpKind,
    rhs: inkwell::values::FloatValue<'ctx>,
    zero: inkwell::values::FloatValue<'ctx>,
    div_zero_bb: inkwell::basic_block::BasicBlock<'ctx>,
    name: &str,
) {
    if op != BinOpKind::Div {
        return;
    }
    let parent = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let cont_bb = self_compiler
        .context
        .append_basic_block(parent, &format!("{name}_div_ok"));
    let is_zero = self_compiler
        .builder
        .build_float_compare(inkwell::FloatPredicate::OEQ, rhs, zero, &format!("{name}_is_zero"))
        .unwrap();
    let _ = self_compiler
        .builder
        .build_conditional_branch(is_zero, div_zero_bb, cont_bb);
    self_compiler.builder.position_at_end(cont_bb);
}

fn create_add_expr_build_float_branch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    module: &inkwell::module::Module<'ctx>,
    l_ptr: PointerValue<'ctx>,
    r_ptr: PointerValue<'ctx>,
    float_tag: IntValue<'ctx>,
    op: BinOpKind,
) -> Result<PointerValue<'ctx>, SprsError> {
    let l_float_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            l_ptr,
            1,
            "l_float_data_ptr",
        )
        .unwrap();
    let l_float_bits = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            l_float_data_ptr,
            "l_float_bits",
        )
        .unwrap()
        .into_int_value();

    let r_float_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            r_ptr,
            1,
            "r_float_data_ptr",
        )
        .unwrap();
    let r_float_bits = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            r_float_data_ptr,
            "r_float_bits",
        )
        .unwrap()
        .into_int_value();

    let parent = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let bb_f16 = self_compiler
        .context
        .append_basic_block(parent, "add_f16_bb");
    let bb_f32 = self_compiler
        .context
        .append_basic_block(parent, "add_f32_bb");
    let bb_f64 = self_compiler
        .context
        .append_basic_block(parent, "add_f64_bb");
    let marge = self_compiler
        .context
        .append_basic_block(parent, "add_merge_bb");
    let final_merge = self_compiler
        .context
        .append_basic_block(parent, "add_float_final_merge_bb");
    let error_bb = self_compiler
        .context
        .append_basic_block(parent, "add_float_error_bb");
    let div_zero_bb = self_compiler
        .context
        .append_basic_block(parent, "float_div_zero_bb");

    let float_tag_const = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float as u64, false);
    let f16_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float16 as u64, false);
    let f32_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float32 as u64, false);
    let f64_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float64 as u64, false);

    let cases = vec![
        (float_tag_const, bb_f64),
        (f16_tag, bb_f16),
        (f32_tag, bb_f32),
        (f64_tag, bb_f64),
    ];

    self_compiler
        .builder
        .build_switch(float_tag, error_bb, &cases)
        .unwrap();

    // error branch (BUG-L17): unknown float tag → error label instead of panic
    self_compiler.builder.position_at_end(error_bb);
    let error_message = "TypeError: unexpected float tag in add";
    let error_ptr = create_error_label_from_str(self_compiler, error_message, module)?;
    let _ = self_compiler.builder.build_unconditional_branch(final_merge);

    self_compiler.builder.position_at_end(div_zero_bb);
    let div_zero_ptr = create_error_label_from_str(self_compiler, "Division by zero", module)?;
    let _ = self_compiler.builder.build_unconditional_branch(final_merge);

    // Float16
    self_compiler.builder.position_at_end(bb_f16);
    let l_i16 = self_compiler
        .builder
        .build_int_truncate(l_float_bits, self_compiler.context.i16_type(), "f16_to_f64")
        .unwrap();
    let l_f16 = self_compiler
        .builder
        .build_bit_cast(l_i16, self_compiler.context.f16_type(), "f16_to_f64_cast")
        .unwrap()
        .into_float_value();

    let r_i16 = self_compiler
        .builder
        .build_int_truncate(r_float_bits, self_compiler.context.i16_type(), "f16_to_f64")
        .unwrap();
    let r_f16 = self_compiler
        .builder
        .build_bit_cast(r_i16, self_compiler.context.f16_type(), "f16_to_f64_cast")
        .unwrap()
        .into_float_value();
    maybe_guard_float_div_zero(
        self_compiler,
        op,
        r_f16,
        self_compiler.context.f16_type().const_float(0.0),
        div_zero_bb,
        "f16",
    );
    let sum_f16 = apply_float_op(self_compiler, op, l_f16, r_f16, "f16_op");
    let sum_i16 = self_compiler
        .builder
        .build_bit_cast(sum_f16, self_compiler.context.i16_type(), "f16_to_i16_cast")
        .unwrap()
        .into_int_value();
    let res_f16_bits = self_compiler
        .builder
        .build_int_s_extend(sum_i16, self_compiler.context.i64_type(), "f16_to_i64")
        .unwrap();

    let f16_end_bb = self_compiler.builder.get_insert_block().unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float32
    self_compiler.builder.position_at_end(bb_f32);
    let l_i32 = self_compiler
        .builder
        .build_int_truncate(l_float_bits, self_compiler.context.i32_type(), "f32_to_f64")
        .unwrap();
    let l_f32 = self_compiler
        .builder
        .build_bit_cast(l_i32, self_compiler.context.f32_type(), "f32_to_f64_cast")
        .unwrap()
        .into_float_value();
    let r_i32 = self_compiler
        .builder
        .build_int_truncate(r_float_bits, self_compiler.context.i32_type(), "f32_to_f64")
        .unwrap();
    let r_f32 = self_compiler
        .builder
        .build_bit_cast(r_i32, self_compiler.context.f32_type(), "f32_to_f64_cast")
        .unwrap()
        .into_float_value();
    maybe_guard_float_div_zero(
        self_compiler,
        op,
        r_f32,
        self_compiler.context.f32_type().const_float(0.0),
        div_zero_bb,
        "f32",
    );
    let sum_f32 = apply_float_op(self_compiler, op, l_f32, r_f32, "f32_op");
    let sum_i32 = self_compiler
        .builder
        .build_bit_cast(sum_f32, self_compiler.context.i32_type(), "f32_to_i32_cast")
        .unwrap()
        .into_int_value();
    let res_f32_bits = self_compiler
        .builder
        .build_int_s_extend(sum_i32, self_compiler.context.i64_type(), "f32_to_i64")
        .unwrap();
    let f32_end_bb = self_compiler.builder.get_insert_block().unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float64
    self_compiler.builder.position_at_end(bb_f64);
    let l_f64 = self_compiler
        .builder
        .build_bit_cast(
            l_float_bits,
            self_compiler.context.f64_type(),
            "l_float_val",
        )
        .unwrap()
        .into_float_value();
    let r_f64 = self_compiler
        .builder
        .build_bit_cast(
            r_float_bits,
            self_compiler.context.f64_type(),
            "r_float_val",
        )
        .unwrap()
        .into_float_value();
    maybe_guard_float_div_zero(
        self_compiler,
        op,
        r_f64,
        self_compiler.context.f64_type().const_float(0.0),
        div_zero_bb,
        "f64",
    );
    let sum_f64 = apply_float_op(self_compiler, op, l_f64, r_f64, "f64_op");

    let res_f64_bits = self_compiler
        .builder
        .build_bit_cast(sum_f64, self_compiler.context.i64_type(), "f64_to_i64_cast")
        .unwrap()
        .into_int_value();
    let f64_end_bb = self_compiler.builder.get_insert_block().unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Marge

    self_compiler.builder.position_at_end(marge);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.context.i64_type(), "float_add_res_phi")
        .unwrap();
    phi.add_incoming(&[
        (&res_f16_bits, f16_end_bb),
        (&res_f32_bits, f32_end_bb),
        (&res_f64_bits, f64_end_bb),
    ]);
    let res_data = phi.as_basic_value().into_int_value();

    let float_res_ptr = create_entry_block_alloca(self_compiler, "float_res_alloc")?;
    self_compiler.build_runtime_value_store(
        float_res_ptr,
        StoreTag::Dynamic(float_tag),
        StoreValue::Int(res_data),
        "float_res",
    );
    let success_end_bb = self_compiler.builder.get_insert_block().unwrap();
    let _ = self_compiler.builder.build_unconditional_branch(final_merge);

    self_compiler.builder.position_at_end(final_merge);
    let final_phi = self_compiler
        .builder
        .build_phi(
            self_compiler.context.ptr_type(AddressSpace::default()),
            "float_add_final_phi",
        )
        .unwrap();
    final_phi.add_incoming(&[
        (&error_ptr, error_bb),
        (&div_zero_ptr, div_zero_bb),
        (&float_res_ptr, success_end_bb),
    ]);
    Ok(final_phi.as_basic_value().into_pointer_value())
}

fn create_add_expr_build_string_branch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_ptr: PointerValue<'ctx>,
    r_ptr: PointerValue<'ctx>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<PointerValue<'ctx>, SprsError> {
    // Load the slab handles from both operands' `data` fields. With the slab
    // ABI, `data` is an i64 handle into the slot pool, not a raw pointer.
    let l_str_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_str_data_ptr")
        .unwrap();
    let l_str_handle = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            l_str_data_ptr,
            "l_str_handle",
        )
        .unwrap()
        .into_int_value();

    let r_str_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_str_data_ptr")
        .unwrap();
    let r_str_handle = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            r_str_data_ptr,
            "r_str_handle",
        )
        .unwrap()
        .into_int_value();

    // Delegate the concatenation to the runtime, which does it in safe Rust
    // (no `l_len + r_len` overflow, no manual memcpy). This eliminates
    // BUG-L02 (heap buffer overflow in string concat) entirely.
    let concat_fn = self_compiler.get_runtime_fn(module, "__string_concat")?;
    let concat_call = self_compiler
        .builder
        .build_call(
            concat_fn,
            &[l_str_handle.into(), r_str_handle.into()],
            "string_concat_call",
        )
        .unwrap();
    let result_handle = match concat_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => return Err(SprsError::Internal { message: "Expected i64 handle from __string_concat".to_string(), location: None }),
    };

    // Pack the new handle into a fresh runtime value of tag String.
    let str_res_ptr = create_entry_block_alloca(self_compiler, "str_res_alloc")?;
    self_compiler.build_runtime_value_store(
        str_res_ptr,
        StoreTag::Int(Tag::String as u64),
        StoreValue::Int(result_handle),
        "str_concat_res",
    );

    Ok(str_res_ptr)
}

fn create_int8_add_logic<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
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
    let l_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_i8 = self_compiler
        .builder
        .build_int_truncate(l_val_i64, self_compiler.context.i8_type(), "l_trunc_i8")
        .unwrap();
    let r_i8 = self_compiler
        .builder
        .build_int_truncate(r_val_i64, self_compiler.context.i8_type(), "r_trunc_i8")
        .unwrap();

    let res_i8 = self_compiler
        .builder
        .build_int_add(l_i8, r_i8, "i8_sum")
        .unwrap();
    let res_i64 = self_compiler
        .builder
        .build_int_s_extend(res_i8, self_compiler.context.i64_type(), "i8_sum_ext")
        .unwrap();
    let res_ptr = create_entry_block_alloca(self_compiler, "int8_add_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Int8 as u64),
        StoreValue::Int(res_i64),
        "int8_add_res",
    );

    Ok(res_ptr.into())
}

fn create_uint8_add_logic<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
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
    let l_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_u8 = self_compiler
        .builder
        .build_int_truncate(l_val_i64, self_compiler.context.i8_type(), "l_trunc_u8")
        .unwrap();
    let r_u8 = self_compiler
        .builder
        .build_int_truncate(r_val_i64, self_compiler.context.i8_type(), "r_trunc_u8")
        .unwrap();

    let res_u8 = self_compiler
        .builder
        .build_int_add(l_u8, r_u8, "u8_sum")
        .unwrap();
    let res_i64 = self_compiler
        .builder
        .build_int_z_extend(res_u8, self_compiler.context.i64_type(), "u8_sum_ext")
        .unwrap();
    let res_ptr = create_entry_block_alloca(self_compiler, "uint8_add_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Uint8 as u64),
        StoreValue::Int(res_i64),
        "uint8_add_res",
    );

    Ok(res_ptr.into())
}

fn create_int16_add_logic<'ctx>(
    _self_compiler: &mut Compiler<'ctx>,
    _lhs: &Spanned<ast::Expr>,
    _rhs: &Spanned<ast::Expr>,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let l_ptr = _self_compiler
        .compile_expr(_lhs, _module)?
        .into_pointer_value();
    let r_ptr = _self_compiler
        .compile_expr(_rhs, _module)?
        .into_pointer_value();

    let l_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_i16 = _self_compiler
        .builder
        .build_int_truncate(l_val_i64, _self_compiler.context.i16_type(), "l_trunc_i16")
        .unwrap();
    let r_i16 = _self_compiler
        .builder
        .build_int_truncate(r_val_i64, _self_compiler.context.i16_type(), "r_trunc_i16")
        .unwrap();

    let res_i16 = _self_compiler
        .builder
        .build_int_add(l_i16, r_i16, "i16_sum")
        .unwrap();
    let res_i64 = _self_compiler
        .builder
        .build_int_s_extend(res_i16, _self_compiler.context.i64_type(), "i16_sum_ext")
        .unwrap();
    let res_ptr = create_entry_block_alloca(_self_compiler, "int16_add_res_alloc")?;
    _self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Int16 as u64),
        StoreValue::Int(res_i64),
        "int16_add_res",
    );

    Ok(res_ptr.into())
}

fn create_uint16_add_logic<'ctx>(
    _self_compiler: &mut Compiler<'ctx>,
    _lhs: &Spanned<ast::Expr>,
    _rhs: &Spanned<ast::Expr>,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let l_ptr = _self_compiler
        .compile_expr(_lhs, _module)?
        .into_pointer_value();
    let r_ptr = _self_compiler
        .compile_expr(_rhs, _module)?
        .into_pointer_value();

    let l_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_u16 = _self_compiler
        .builder
        .build_int_truncate(l_val_i64, _self_compiler.context.i16_type(), "l_trunc_u16")
        .unwrap();
    let r_u16 = _self_compiler
        .builder
        .build_int_truncate(r_val_i64, _self_compiler.context.i16_type(), "r_trunc_u16")
        .unwrap();

    let res_u16 = _self_compiler
        .builder
        .build_int_add(l_u16, r_u16, "u16_sum")
        .unwrap();
    let res_i64 = _self_compiler
        .builder
        .build_int_z_extend(res_u16, _self_compiler.context.i64_type(), "u16_sum_ext")
        .unwrap();
    let res_ptr = create_entry_block_alloca(_self_compiler, "uint16_add_res_alloc")?;
    _self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Uint16 as u64),
        StoreValue::Int(res_i64),
        "uint16_add_res",
    );

    Ok(res_ptr.into())
}

fn create_int32_add_logic<'ctx>(
    _self_compiler: &mut Compiler<'ctx>,
    _lhs: &Spanned<ast::Expr>,
    _rhs: &Spanned<ast::Expr>,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let l_ptr = _self_compiler
        .compile_expr(_lhs, _module)?
        .into_pointer_value();
    let r_ptr = _self_compiler
        .compile_expr(_rhs, _module)?
        .into_pointer_value();

    let l_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_i32 = _self_compiler
        .builder
        .build_int_truncate(l_val_i64, _self_compiler.context.i32_type(), "l_trunc_i32")
        .unwrap();
    let r_i32 = _self_compiler
        .builder
        .build_int_truncate(r_val_i64, _self_compiler.context.i32_type(), "r_trunc_i32")
        .unwrap();

    let res_i32 = _self_compiler
        .builder
        .build_int_add(l_i32, r_i32, "i32_sum")
        .unwrap();
    let res_i64 = _self_compiler
        .builder
        .build_int_s_extend(res_i32, _self_compiler.context.i64_type(), "i32_sum_ext")
        .unwrap();
    let res_ptr = create_entry_block_alloca(_self_compiler, "int32_add_res_alloc")?;
    _self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Int32 as u64),
        StoreValue::Int(res_i64),
        "int32_add_res",
    );

    Ok(res_ptr.into())
}

fn create_uint32_add_logic<'ctx>(
    _self_compiler: &mut Compiler<'ctx>,
    _lhs: &Spanned<ast::Expr>,
    _rhs: &Spanned<ast::Expr>,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let l_ptr = _self_compiler
        .compile_expr(_lhs, _module)?
        .into_pointer_value();
    let r_ptr = _self_compiler
        .compile_expr(_rhs, _module)?
        .into_pointer_value();

    let l_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_u32 = _self_compiler
        .builder
        .build_int_truncate(l_val_i64, _self_compiler.context.i32_type(), "l_trunc_u32")
        .unwrap();
    let r_u32 = _self_compiler
        .builder
        .build_int_truncate(r_val_i64, _self_compiler.context.i32_type(), "r_trunc_u32")
        .unwrap();

    let res_u32 = _self_compiler
        .builder
        .build_int_add(l_u32, r_u32, "u32_sum")
        .unwrap();
    let res_i64 = _self_compiler
        .builder
        .build_int_z_extend(res_u32, _self_compiler.context.i64_type(), "u32_sum_ext")
        .unwrap();
    let res_ptr = create_entry_block_alloca(_self_compiler, "uint32_add_res_alloc")?;
    _self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Uint32 as u64),
        StoreValue::Int(res_i64),
        "uint32_add_res",
    );

    Ok(res_ptr.into())
}

fn create_int64_add_logic<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
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

    let res_val = self_compiler
        .builder
        .build_int_add(l_val, r_val, "i64_sum")
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "int64_add_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Int64 as u64),
        StoreValue::Int(res_val),
        "int64_add_res",
    );

    Ok(res_ptr.into())
}

fn create_uint64_add_logic<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
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

    let res_val = self_compiler
        .builder
        .build_int_add(l_val, r_val, "u64_sum")
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "uint64_add_res_alloc")?;

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Uint64 as u64),
        StoreValue::Int(res_val),
        "uint64_add_res",
    );
    Ok(res_ptr.into())
}

fn create_float16_add_logic<'ctx>(
    _self_compiler: &mut Compiler<'ctx>,
    _lhs: &Spanned<ast::Expr>,
    _rhs: &Spanned<ast::Expr>,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    let l_ptr = _self_compiler
        .compile_expr(_lhs, _module)?
        .into_pointer_value();
    let r_ptr = _self_compiler
        .compile_expr(_rhs, _module)?
        .into_pointer_value();

    let l_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, l_ptr, 1, "l_data_ptr")
        .unwrap();
    let l_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = _self_compiler
        .builder
        .build_load(_self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_i16 = _self_compiler
        .builder
        .build_int_truncate(l_val_i64, _self_compiler.context.i16_type(), "l_trunc_i16")
        .unwrap();
    let l_f16 = _self_compiler
        .builder
        .build_bit_cast(l_i16, _self_compiler.context.f16_type(), "l_i64_to_f16")
        .unwrap()
        .into_float_value();

    let r_i16 = _self_compiler
        .builder
        .build_int_truncate(r_val_i64, _self_compiler.context.i16_type(), "r_trunc_i16")
        .unwrap();
    let r_f16 = _self_compiler
        .builder
        .build_bit_cast(r_i16, _self_compiler.context.f16_type(), "r_i64_to_f16")
        .unwrap()
        .into_float_value();

    let res_f16 = _self_compiler
        .builder
        .build_float_add(l_f16, r_f16, "f16_sum")
        .unwrap();
    let res_i16 = _self_compiler
        .builder
        .build_bit_cast(res_f16, _self_compiler.context.i16_type(), "f16_sum_to_i16")
        .unwrap()
        .into_int_value();
    let res_i64 = _self_compiler
        .builder
        .build_int_s_extend(res_i16, _self_compiler.context.i64_type(), "f16_sum_to_i64")
        .unwrap();
    let res_ptr = create_entry_block_alloca(_self_compiler, "float16_add_res_alloc")?;
    _self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Float16 as u64),
        StoreValue::Int(res_i64),
        "float16_add_res",
    );

    Ok(res_ptr.into())
}

fn create_float32_add_logic<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
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
    let l_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_i32 = self_compiler
        .builder
        .build_int_truncate(l_val_i64, self_compiler.context.i32_type(), "l_f32_to_i32")
        .unwrap();

    let l_f32 = self_compiler
        .builder
        .build_bit_cast(l_i32, self_compiler.context.f32_type(), "l_i64_to_f32")
        .unwrap()
        .into_float_value();

    let r_i32 = self_compiler
        .builder
        .build_int_truncate(r_val_i64, self_compiler.context.i32_type(), "r_f32_to_i32")
        .unwrap();

    let r_f32 = self_compiler
        .builder
        .build_bit_cast(r_i32, self_compiler.context.f32_type(), "r_i64_to_f32")
        .unwrap()
        .into_float_value();

    let res_f32 = self_compiler
        .builder
        .build_float_add(l_f32, r_f32, "f32_sum")
        .unwrap();

    let res_i32 = self_compiler
        .builder
        .build_bit_cast(res_f32, self_compiler.context.i32_type(), "f32_sum_to_i32")
        .unwrap()
        .into_int_value();
    let res_i64 = self_compiler
        .builder
        .build_int_z_extend(res_i32, self_compiler.context.i64_type(), "f32_sum_to_i64")
        .unwrap();
    let res_ptr = create_entry_block_alloca(self_compiler, "float32_add_res_alloc")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Float32 as u64),
        StoreValue::Int(res_i64),
        "float32_add_res",
    );

    Ok(res_ptr.into())
}

fn create_float64_add_logic<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
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
    let l_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), l_data_ptr, "l_val_i64")
        .unwrap()
        .into_int_value();

    let r_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_data_ptr")
        .unwrap();
    let r_val_i64 = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), r_data_ptr, "r_val_i64")
        .unwrap()
        .into_int_value();

    let l_f64 = self_compiler
        .builder
        .build_bit_cast(l_val_i64, self_compiler.context.f64_type(), "l_i64_to_f64")
        .unwrap()
        .into_float_value();
    let r_f64 = self_compiler
        .builder
        .build_bit_cast(r_val_i64, self_compiler.context.f64_type(), "r_i64_to_f64")
        .unwrap()
        .into_float_value();

    let res_f64 = self_compiler
        .builder
        .build_float_add(l_f64, r_f64, "f64_sum")
        .unwrap();
    let res_i64 = self_compiler
        .builder
        .build_bit_cast(res_f64, self_compiler.context.i64_type(), "f64_sum_to_i64")
        .unwrap()
        .into_int_value();

    let res_ptr = create_entry_block_alloca(self_compiler, "float64_add_res_alloc")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Float64 as u64),
        StoreValue::Int(res_i64),
        "float64_add_res",
    );

    Ok(res_ptr.into())
}

pub fn create_mul_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_binary_dispatch(self_compiler, lhs, rhs, module, BinOpKind::Mul)
}

pub fn create_minus_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_binary_dispatch(self_compiler, lhs, rhs, module, BinOpKind::Sub)
}

pub fn create_div_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_binary_dispatch(self_compiler, lhs, rhs, module, BinOpKind::Div)
}

pub fn create_mod_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &Spanned<ast::Expr>,
    rhs: &Spanned<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    create_binary_dispatch(self_compiler, lhs, rhs, module, BinOpKind::Mod)
}

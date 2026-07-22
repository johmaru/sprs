use inkwell::{
    AddressSpace,
    values::{BasicValueEnum, IntValue, PointerValue, ValueKind},
};
use crate::{
    front::ast,
    front::type_helper,
    llvm::compiler::{Compiler, StoreTag, StoreValue, Tag},
};
use crate::llvm::value::{create_panic_err, create_entry_block_alloca, PanicErrorSettings};
use crate::llvm::variable::move_variable;

pub fn create_add_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    if let Ok(val) = create_add_expr_type_check(self_compiler, lhs, rhs, module) {
        return Ok(val);
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

    // check if both are integers

    let can_add = create_add_expr_check_int(self_compiler, l_tag, r_tag)?;

    // check if both are float(default(f64))
    let both_float = create_add_expr_check_float(self_compiler, l_tag, r_tag)?;

    // check if both are strings
    let check_string = create_add_expr_check_string(self_compiler, l_tag, r_tag)?;

    // create branches
    let parent_fn = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let int_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "add_int_bb");
    let check_float_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "add_check_float_bb");
    let float_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "add_float_bb");
    let check_string_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "add_check_string_bb");
    let string_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "add_string_bb");
    let error_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "add_error_bb");

    let merge_bb = self_compiler
        .context
        .append_basic_block(parent_fn, "add_merge_bb");

    // first check if can add as integers
    let _ = self_compiler
        .builder
        .build_conditional_branch(can_add, int_bb, check_float_bb);

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
        self_compiler.get_known_type_from_expr(lhs),
        self_compiler.get_known_type_from_expr(rhs)
    );

    let settings = PanicErrorSettings {
        is_const: true,
        is_global: true,
    };

    let _ = create_panic_err(
        self_compiler,
        Box::leak(error_message.into_boxed_str()), // error message has memory leak but it's acceptable for now
        module,
        settings,
    )?;

    let _ = self_compiler.builder.build_unreachable();

    // integer addition branch

    self_compiler.builder.position_at_end(int_bb);

    let int_res_ptr = create_add_expr_build_int_branch(self_compiler, l_ptr, r_ptr, l_tag)?;
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    // float addition branch

    self_compiler.builder.position_at_end(float_bb);

    let float_res_ptr = create_add_expr_build_float_branch(self_compiler, l_ptr, r_ptr, l_tag)?;
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
        (&int_res_ptr, int_bb),
        (&float_res_ptr, float_end_bb),
        (&str_res_ptr, string_bb),
    ]);

    Ok(phi.as_basic_value())
}

fn create_add_expr_type_check<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let is_type = |expr: &ast::Expr, ty: &str| -> bool {
        match self_compiler.get_known_type_from_expr(expr) {
            Ok(t) => t == ty,
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

    Err("Unsupported types for addition".to_string())
}

fn create_add_expr_check_int<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_tag: IntValue<'ctx>,
    r_tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, String> {
    let int_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Integer as u64, false);
    let int8_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Int8 as u64, false);
    let uint8_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Uint8 as u64, false);
    let int16_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Int16 as u64, false);
    let uint16_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Uint16 as u64, false);
    let int32_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Int32 as u64, false);
    let uint32_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Uint32 as u64, false);
    let int64_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Int64 as u64, false);
    let uint64_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Uint64 as u64, false);
    let tags_equal = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, r_tag, "tags_equal")
        .unwrap();

    let is_l_int = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, int_tag, "is_l_int")
        .unwrap();
    let is_l_int8 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, int8_tag, "is_l_int8")
        .unwrap();
    let is_l_uint8 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, uint8_tag, "is_l_uint8")
        .unwrap();
    let is_l_int16 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, int16_tag, "is_l_int16")
        .unwrap();
    let is_l_uint16 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, uint16_tag, "is_l_uint16")
        .unwrap();
    let is_l_int32 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, int32_tag, "is_l_int32")
        .unwrap();
    let is_l_uint32 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, uint32_tag, "is_l_uint32")
        .unwrap();
    let is_l_int64 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, int64_tag, "is_l_int64")
        .unwrap();
    let is_l_uint64 = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, uint64_tag, "is_l_uint64")
        .unwrap();
    let is_l_numeric = self_compiler
        .builder
        .build_or(is_l_int, is_l_int8, "is_l_numeric")
        .unwrap();
    let is_l_numeric_1 = self_compiler
        .builder
        .build_or(is_l_uint8, is_l_numeric, "is_l_numeric_1")
        .unwrap();
    let is_l_numeric_2 = self_compiler
        .builder
        .build_or(is_l_int16, is_l_numeric_1, "is_l_numeric_2")
        .unwrap();
    let is_l_numeric_3 = self_compiler
        .builder
        .build_or(is_l_uint16, is_l_numeric_2, "is_l_numeric_3")
        .unwrap();
    let is_l_numeric_4 = self_compiler
        .builder
        .build_or(is_l_int32, is_l_numeric_3, "is_l_numeric_4")
        .unwrap();
    let is_l_numeric_5 = self_compiler
        .builder
        .build_or(is_l_uint32, is_l_numeric_4, "is_l_numeric_5")
        .unwrap();
    let is_l_numeric_6 = self_compiler
        .builder
        .build_or(is_l_int64, is_l_numeric_5, "is_l_numeric_6")
        .unwrap();
    let is_l_numeric_final = self_compiler
        .builder
        .build_or(is_l_uint64, is_l_numeric_6, "is_l_numeric_final")
        .unwrap();

    let can_add = self_compiler
        .builder
        .build_and(tags_equal, is_l_numeric_final, "can_add")
        .unwrap();

    Ok(can_add)
}

// currently only handling int + int and string + string, for now didn't use a both_string variable
// 0 isBothString , 1 tag
fn create_add_expr_check_string<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_tag: IntValue<'ctx>,
    r_tag: IntValue<'ctx>,
) -> Result<IntValue<'ctx>, String> {
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
) -> Result<IntValue<'ctx>, String> {
    let float_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float as u64, false);
    let float16_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float16 as u64, false);
    let float32_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float32 as u64, false);
    let float64_tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Float64 as u64, false);
    let float_tags_equal = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, r_tag, "float_tags_equal")
        .unwrap();

    let is_l_float = self_compiler
        .builder
        .build_int_compare(inkwell::IntPredicate::EQ, l_tag, float_tag, "is_l_float")
        .unwrap();

    let is_float_1 = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            l_tag,
            float16_tag,
            "is_l_float16",
        )
        .unwrap();
    let is_float_2 = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            l_tag,
            float32_tag,
            "is_l_float32",
        )
        .unwrap();
    let is_float_3 = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            l_tag,
            float64_tag,
            "is_l_float64",
        )
        .unwrap();

    let is_float_combined_1 = self_compiler
        .builder
        .build_or(is_l_float, is_float_1, "is_l_float_combined_1")
        .unwrap();
    let is_float_combined_2 = self_compiler
        .builder
        .build_or(is_float_2, is_float_combined_1, "is_l_float_combined_2")
        .unwrap();
    let is_l_float_final = self_compiler
        .builder
        .build_or(is_float_3, is_float_combined_2, "is_l_float_final")
        .unwrap();

    let both_float = self_compiler
        .builder
        .build_and(float_tags_equal, is_l_float_final, "both_float")
        .unwrap();

    Ok(both_float)
}

fn create_add_expr_build_int_branch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_ptr: PointerValue<'ctx>,
    r_ptr: PointerValue<'ctx>,
    l_tag: IntValue<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
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

    let int_sum = self_compiler
        .builder
        .build_int_add(l_int_val, r_int_val, "int_sum")
        .unwrap();

    let int_res_ptr = create_entry_block_alloca(self_compiler, "int_res_alloc");
    self_compiler.build_runtime_value_store(
        int_res_ptr,
        StoreTag::Dynamic(l_tag),
        StoreValue::Int(int_sum),
        "int_res",
    );

    Ok(int_res_ptr)
}

fn create_add_expr_build_float_branch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_ptr: PointerValue<'ctx>,
    r_ptr: PointerValue<'ctx>,
    float_tag: IntValue<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
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

    let cases = vec![(f16_tag, bb_f16), (f32_tag, bb_f32), (f64_tag, bb_f64)];

    self_compiler
        .builder
        .build_switch(float_tag, bb_f64, &cases)
        .unwrap();

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
    let sum_f16 = self_compiler
        .builder
        .build_float_add(l_f16, r_f16, "f16_add")
        .unwrap();
    let sum_i16 = self_compiler
        .builder
        .build_bit_cast(sum_f16, self_compiler.context.i16_type(), "f16_to_i16_cast")
        .unwrap()
        .into_int_value();
    let res_f16_bits = self_compiler
        .builder
        .build_int_s_extend(sum_i16, self_compiler.context.i64_type(), "f16_to_i64")
        .unwrap();

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
    let sum_f32 = self_compiler
        .builder
        .build_float_add(l_f32, r_f32, "f32_add")
        .unwrap();
    let sum_i32 = self_compiler
        .builder
        .build_bit_cast(sum_f32, self_compiler.context.i32_type(), "f32_to_i32_cast")
        .unwrap()
        .into_int_value();
    let res_f32_bits = self_compiler
        .builder
        .build_int_s_extend(sum_i32, self_compiler.context.i64_type(), "f32_to_i64")
        .unwrap();
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
    let sum_f64 = self_compiler
        .builder
        .build_float_add(l_f64, r_f64, "f64_add")
        .unwrap();

    let res_f64_bits = self_compiler
        .builder
        .build_bit_cast(sum_f64, self_compiler.context.i64_type(), "f64_to_i64_cast")
        .unwrap()
        .into_int_value();
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
        (&res_f16_bits, bb_f16),
        (&res_f32_bits, bb_f32),
        (&res_f64_bits, bb_f64),
    ]);
    let res_data = phi.as_basic_value().into_int_value();

    let float_res_ptr = create_entry_block_alloca(self_compiler, "float_res_alloc");
    self_compiler.build_runtime_value_store(
        float_res_ptr,
        StoreTag::Dynamic(float_tag),
        StoreValue::Int(res_data),
        "float_res",
    );
    Ok(float_res_ptr)
}

fn create_add_expr_build_string_branch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_ptr: PointerValue<'ctx>,
    r_ptr: PointerValue<'ctx>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
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
    let concat_fn = self_compiler.get_runtime_fn(module, "__string_concat");
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
        _ => return Err("Expected i64 handle from __string_concat".to_string()),
    };

    // Pack the new handle into a fresh runtime value of tag String.
    let str_res_ptr = create_entry_block_alloca(self_compiler, "str_res_alloc");
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
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(self_compiler, "int8_add_res_alloc");

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
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(self_compiler, "uint8_add_res_alloc");

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
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(_self_compiler, "int16_add_res_alloc");
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
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(_self_compiler, "uint16_add_res_alloc");
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
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(_self_compiler, "int32_add_res_alloc");
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
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(_self_compiler, "uint32_add_res_alloc");
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
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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

    let res_ptr = create_entry_block_alloca(self_compiler, "int64_add_res_alloc");

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
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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

    let res_ptr = create_entry_block_alloca(self_compiler, "uint64_add_res_alloc");

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
    _lhs: &ast::Expr,
    _rhs: &ast::Expr,
    _module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(_self_compiler, "float16_add_res_alloc");
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
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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
    let res_ptr = create_entry_block_alloca(self_compiler, "float32_add_res_alloc");
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
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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

    let res_ptr = create_entry_block_alloca(self_compiler, "float64_add_res_alloc");
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
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(
        self_compiler,
        lhs,
        rhs,
        module,
        IntBinOp::Mul,
        |builder, l_val, r_val, name| Ok(builder.build_int_mul(l_val, r_val, name).unwrap()),
    )
}

pub fn create_minus_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(
        self_compiler,
        lhs,
        rhs,
        module,
        IntBinOp::Sub,
        |builder, l_val, r_val, name| Ok(builder.build_int_sub(l_val, r_val, name).unwrap()),
    )
}

pub fn create_div_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(
        self_compiler,
        lhs,
        rhs,
        module,
        IntBinOp::Div,
        |builder, l_val, r_val, name| Ok(builder.build_int_signed_div(l_val, r_val, name).unwrap()),
    )
}

pub fn create_mod_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(
        self_compiler,
        lhs,
        rhs,
        module,
        IntBinOp::Mod,
        |builder, l_val, r_val, name| Ok(builder.build_int_signed_rem(l_val, r_val, name).unwrap()),
    )
}

enum IntBinOp {
    Sub,
    Mul,
    Div,
    Mod,
}

fn create_binary_int_op<'ctx, F>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
    op: IntBinOp,
    op_fn: F,
) -> Result<BasicValueEnum<'ctx>, String>
where
    F: Fn(
        &inkwell::builder::Builder<'ctx>,
        inkwell::values::IntValue<'ctx>,
        inkwell::values::IntValue<'ctx>,
        &str,
    ) -> Result<inkwell::values::IntValue<'ctx>, String>,
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
        match op {
            IntBinOp::Sub => "difference",
            IntBinOp::Mul => "product",
            IntBinOp::Div => "quotient",
            IntBinOp::Mod => "remainder",
        },
    )?;

    let res_ptr = create_entry_block_alloca(self_compiler, "res_alloc");

    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Integer as u64),
        StoreValue::Int(result),
        "int_bin_op_res",
    );
    Ok(res_ptr.into())
}

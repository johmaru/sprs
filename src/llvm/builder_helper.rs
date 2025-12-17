use core::error;

use inkwell::{
    AddressSpace,
    builder::Builder,
    module::Linkage,
    values::{BasicValueEnum, FunctionValue, IntValue, PointerValue, ValueKind},
};

use crate::{
    front::ast,
    llvm::compiler::{Compiler, Tag},
};

// !support functions
pub struct PanicErrorSettings {
    pub is_const: bool,
    pub is_global: bool,
}
pub fn create_panic_err<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    message: &str,
    module: &inkwell::module::Module<'ctx>,
    settings: PanicErrorSettings,
) -> Result<(), String> {
    let global = if let Some(existing) = self_compiler.string_constants.get(message) {
        *existing
    } else {
        let str_val = self_compiler.context.const_string(message.as_bytes(), true);
        let global = module.add_global(
            str_val.get_type(),
            Some(AddressSpace::default()),
            &format!("panic_err_{}", self_compiler.string_constants.len()),
        );
        global.set_initializer(&str_val);
        if settings.is_const {
            global.set_constant(true);
        }
        if settings.is_global {
            global.set_linkage(Linkage::External);
        } else {
            global.set_linkage(Linkage::Internal);
        }
        self_compiler
            .string_constants
            .insert(message.to_string(), global);
        global
    };

    let str_ptr = global.as_pointer_value();
    let str_ptr_i8 = self_compiler.builder.build_bit_cast(
        str_ptr,
        self_compiler.context.ptr_type(AddressSpace::default()),
        "panic_err_str_ptr_i8",
    );

    let panic_fn = self_compiler.get_runtime_fn(module, "__panic");
    self_compiler
        .builder
        .build_call(panic_fn, &[str_ptr_i8.unwrap().into()], "panic_call")
        .unwrap();
    Ok(())
}

fn create_entry_block_alloca<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    name: &str,
) -> PointerValue<'ctx> {
    let builder = &self_compiler.builder;
    let current_block = builder.get_insert_block().unwrap();
    let function = current_block.get_parent().unwrap();
    let entry_block = function.get_first_basic_block().unwrap();

    match entry_block.get_first_instruction() {
        Some(first_instr) => builder.position_before(&first_instr),
        None => builder.position_at_end(entry_block),
    }

    let alloca = builder
        .build_alloca(
            self_compiler.runtime_value_type,
            format!("{}_var_alloca", name).as_str(),
        )
        .unwrap();

    builder.position_at_end(current_block);
    alloca
}

// !normal functions

pub fn create_list_from_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    elements: &[ast::Expr],
    module: &inkwell::module::Module<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
    let len = elements.len();
    let i64_type = self_compiler.context.i64_type();

    let list_new_fn = self_compiler.get_runtime_fn(module, "__list_new");

    let list_ptr = self_compiler
        .builder
        .build_call(
            list_new_fn,
            &[i64_type.const_int(len as u64, false).into()],
            "list_ptr",
        )
        .unwrap();

    let list_ptr_val = match list_ptr.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_pointer_value(),
        _ => return Err("Expected a basic value".to_string()),
    };

    let list_push_fn = self_compiler.get_runtime_fn(module, "__list_push");
    for elem in elements {
        let val_ptr = self_compiler
            .compile_expr(elem, module)?
            .into_pointer_value();

        let target_ptr = self_compiler
            .builder
            .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 0, "val_tag_ptr")
            .unwrap();
        let val_tag = self_compiler
            .builder
            .build_load(self_compiler.context.i32_type(), target_ptr, "val_tag")
            .unwrap()
            .into_int_value();

        let data_ptr = self_compiler
            .builder
            .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 1, "val_data_ptr")
            .unwrap();
        let val_data = self_compiler
            .builder
            .build_load(self_compiler.context.i64_type(), data_ptr, "val_data")
            .unwrap()
            .into_int_value();

        self_compiler
            .builder
            .build_call(
                list_push_fn,
                &[list_ptr_val.into(), val_tag.into(), val_data.into()],
                "list_push_call",
            )
            .unwrap();
    }
    Ok(list_ptr_val)
}

// A runtime move system for variables that hold heap data (strings, lists, ranges)
// When passing such variables to functions, we need to "move" them by resetting their tag to Unit
// If want to keep the data, can use "clone" macro.
pub fn move_variable<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    src_enum_ptr: &BasicValueEnum<'ctx>,
    name: &str,
) {
    let src_ptr = src_enum_ptr.into_pointer_value();

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            src_ptr,
            0,
            &format!("{}_tag_ptr", name),
        )
        .unwrap();

    let current_tag = self_compiler
        .builder
        .build_load(
            self_compiler.context.i32_type(),
            tag_ptr,
            &format!("{}_current_tag", name),
        )
        .unwrap()
        .into_int_value();

    let tag_string = self_compiler
        .context
        .i32_type()
        .const_int(Tag::String as u64, false);
    let tag_list = self_compiler
        .context
        .i32_type()
        .const_int(Tag::List as u64, false);
    let tag_range = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Range as u64, false);
    let is_string = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            current_tag,
            tag_string,
            &format!("{}_is_string", name),
        )
        .unwrap();
    let is_list = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            current_tag,
            tag_list,
            &format!("{}_is_list", name),
        )
        .unwrap();
    let is_range = self_compiler
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            current_tag,
            tag_range,
            &format!("{}_is_range", name),
        )
        .unwrap();

    let is_heap_1 = self_compiler
        .builder
        .build_or(is_string, is_list, &format!("{}_is_heap_1", name))
        .unwrap();
    let should_move = self_compiler
        .builder
        .build_or(is_heap_1, is_range, &format!("{}_should_move", name))
        .unwrap();

    let parent_bb = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();
    let move_bb = self_compiler
        .context
        .append_basic_block(parent_bb, &format!("{}_move_bb", name));
    let cont_bb = self_compiler
        .context
        .append_basic_block(parent_bb, &format!("{}_cont_bb", name));

    let _ = self_compiler
        .builder
        .build_conditional_branch(should_move, move_bb, cont_bb);

    self_compiler.builder.position_at_end(move_bb);
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Unit as u64, false),
        )
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(cont_bb)
        .unwrap();
    self_compiler.builder.position_at_end(cont_bb);
}

pub fn load_at_init_variable_with_existing<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    init_value: PointerValue<'ctx>,
    ptr: PointerValue<'ctx>,
    name: &str,
) {
    let val = self_compiler
        .builder
        .build_load(
            self_compiler.runtime_value_type,
            init_value,
            &format!("{}_assign_load", name),
        )
        .unwrap();
    let _ = self_compiler.builder.build_store(ptr, val);
}

pub fn var_load_at_init_variable<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    init_value: PointerValue<'ctx>,
    name: &str,
) -> PointerValue<'ctx> {
    let ptr = create_entry_block_alloca(self_compiler, name);

    let val = self_compiler
        .builder
        .build_load(
            self_compiler.runtime_value_type,
            init_value,
            &format!("{}_var_load", name),
        )
        .unwrap();
    let _ = self_compiler.builder.build_store(ptr, val).unwrap();
    ptr
}

pub fn var_return_store<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    value_enum: &BasicValueEnum<'ctx>,
    name: &str,
) {
    let var_ptr = value_enum.into_pointer_value();

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            var_ptr,
            0,
            &format!("{}_tag_ptr", name),
        )
        .unwrap();

    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Unit as u64, false),
        )
        .unwrap();
}

pub fn drop_var<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    ptr: PointerValue<'ctx>,
    drop_fn: FunctionValue<'_>,
    name: &str,
) {
    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "var_tag_ptr")
        .unwrap();
    let tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), tag_ptr, "var_tag")
        .unwrap()
        .into_int_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "var_data_ptr")
        .unwrap();
    let data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "var_data")
        .unwrap()
        .into_int_value();

    self_compiler
        .builder
        .build_call(drop_fn, &[tag.into(), data.into()], "drop_var_call")
        .unwrap();
}

pub fn create_dummy_for_no_return<'ctx>(self_compiler: &mut Compiler<'ctx>) {
    let dummy = create_entry_block_alloca(self_compiler, "ret_dummy");
    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, dummy, 0, "ret_dummy_tag")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Unit as u64, false),
        )
        .unwrap();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, dummy, 1, "ret_dummy_data")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    let val = self_compiler
        .builder
        .build_load(self_compiler.runtime_value_type, dummy, "ret_dummy_val")
        .unwrap();
    self_compiler.builder.build_return(Some(&val)).unwrap();
}

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

pub fn create_integer<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    n: &i64,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "num_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Integer as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(*n as u64, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_float<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    f: f64,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "float_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "data_ptr")
        .unwrap();
    let float_bits = f.to_bits();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler
                .context
                .i64_type()
                .const_int(float_bits, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_string<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    str: &String,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let global = if let Some(existing) = self_compiler.string_constants.get(str) {
        *existing
    } else {
        let str_val = self_compiler.context.const_string(str.as_bytes(), true);
        let global = module.add_global(
            str_val.get_type(),
            Some(AddressSpace::default()),
            &format!("str_const_{}", self_compiler.string_constants.len()),
        );
        global.set_initializer(&str_val);
        global.set_linkage(Linkage::Internal);
        global.set_constant(true);
        self_compiler.string_constants.insert(str.clone(), global);
        global
    };

    let ptr = create_entry_block_alloca(self_compiler, "str_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::String as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "data_ptr")
        .unwrap();
    let str_ptr = global.as_pointer_value();
    let str_ptr_as_i64 = self_compiler
        .builder
        .build_ptr_to_int(str_ptr, self_compiler.context.i64_type(), "str_ptr_as_i64")
        .unwrap();
    self_compiler
        .builder
        .build_store(data_ptr, str_ptr_as_i64)
        .unwrap();

    Ok(ptr.into())
}

pub fn create_bool<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    boolean: &bool,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "bool_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "bool_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Boolean as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "bool_data_ptr")
        .unwrap();
    let bool_val = if *boolean { 1 } else { 0 };
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(bool_val, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_int8<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "int8_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "int8_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int8 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "int8_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_uint8<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "uint8_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "uint8_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint8 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "uint8_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_int16<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "int16_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "int16_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int16 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "int16_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_uint16<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "uint16_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "uint16_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint16 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "uint16_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_int32<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "int32_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "int32_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int32 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "int32_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_uint32<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "uint32_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "uint32_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint32 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "uint32_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_int64<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "int64_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "int64_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int64 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "int64_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_uint64<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "uint64_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "uint64_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint64 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "uint64_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_float16<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "f16_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "f16_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float16 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "f16_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_float32<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "f32_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "f32_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float32 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "f32_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

pub fn create_float64<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, "f64_alloc");

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "f64_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float64 as u64, false),
        )
        .unwrap();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "f64_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            data_ptr,
            self_compiler.context.i64_type().const_int(0, false),
        )
        .unwrap();

    Ok(ptr.into())
}

fn box_return_value<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    return_type: inkwell::types::BasicTypeEnum<'ctx>,
    result_val: BasicValueEnum<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_ptr = create_entry_block_alloca(self_compiler, "compile_expr_call_res_alloc");

    if return_type.is_int_type() {
        let int_val = result_val.into_int_value();

        let val_i64 = self_compiler
            .builder
            .build_int_s_extend(int_val, self_compiler.context.i64_type(), "int_to_i64")
            .unwrap();

        let tag_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                result_ptr,
                0,
                "res_tag_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(
                tag_ptr,
                self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::Integer as u64, false),
            )
            .unwrap();

        let data_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                result_ptr,
                1,
                "res_data_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(data_ptr, val_i64)
            .unwrap();
    } else if return_type.is_float_type() {
        let float_val = result_val.into_float_value();

        let val_f64 = self_compiler
            .builder
            .build_float_ext(float_val, self_compiler.context.f64_type(), "float_to_f64")
            .unwrap();

        let data = self_compiler
            .builder
            .build_bit_cast(val_f64, self_compiler.context.i64_type(), "f64_to_i64")
            .unwrap()
            .into_int_value();

        let tag_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                result_ptr,
                0,
                "res_tag_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(
                tag_ptr,
                self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::Float as u64, false),
            )
            .unwrap();

        let data_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                result_ptr,
                1,
                "res_data_ptr",
            )
            .unwrap();
        self_compiler.builder.build_store(data_ptr, data).unwrap();
    } else if return_type.is_struct_type() {
        self_compiler
            .builder
            .build_store(result_ptr, result_val)
            .unwrap();
    } else if return_type.is_pointer_type() {
        let ptr_val = result_val.into_pointer_value();
        let ptr_as_i64 = self_compiler
            .builder
            .build_ptr_to_int(ptr_val, self_compiler.context.i64_type(), "ptr_to_i64")
            .unwrap();

        let tag_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                result_ptr,
                0,
                "res_tag_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(
                tag_ptr,
                self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::String as u64, false),
            )
            .unwrap();

        let data_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                result_ptr,
                1,
                "res_data_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(data_ptr, ptr_as_i64)
            .unwrap();
    } else {
        let tag_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                result_ptr,
                0,
                "res_tag_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(
                tag_ptr,
                self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::Unit as u64, false),
            )
            .unwrap();
    };
    Ok(result_ptr.into())
}

pub fn create_call_expr<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    ident: &str,
    args: &Vec<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let func = module
        .get_function(ident)
        .or_else(|| {
            self_compiler
                .modules
                .values()
                .find_map(|m| m.get_function(ident))
        })
        .ok_or(format!("Undefined function: {}", ident))?;
    let mut compiled_args = Vec::with_capacity(args.len());
    for arg in args {
        let arg_val = self_compiler.compile_expr(arg, module)?;
        let arg_ptr = arg_val.into_pointer_value();

        let temp_arg_ptr = create_entry_block_alloca(self_compiler, "compile_expr_arg_alloc");
        let val_tag_ptr = self_compiler
            .builder
            .build_struct_gep(self_compiler.runtime_value_type, arg_ptr, 0, "val_tag_ptr")
            .unwrap();
        let val_data_ptr = self_compiler
            .builder
            .build_struct_gep(self_compiler.runtime_value_type, arg_ptr, 1, "val_data_ptr")
            .unwrap();
        let val_tag = self_compiler
            .builder
            .build_load(self_compiler.context.i32_type(), val_tag_ptr, "val_tag")
            .unwrap();
        let val_data = self_compiler
            .builder
            .build_load(self_compiler.context.i64_type(), val_data_ptr, "val_data")
            .unwrap();

        let temp_tag_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                temp_arg_ptr,
                0,
                "temp_tag_ptr",
            )
            .unwrap();
        let temp_data_ptr = self_compiler
            .builder
            .build_struct_gep(
                self_compiler.runtime_value_type,
                temp_arg_ptr,
                1,
                "temp_data_ptr",
            )
            .unwrap();
        self_compiler
            .builder
            .build_store(temp_tag_ptr, val_tag)
            .unwrap();
        self_compiler
            .builder
            .build_store(temp_data_ptr, val_data)
            .unwrap();
        compiled_args.push(temp_arg_ptr.into());

        if let ast::Expr::Var(name) = arg {
            if let Some((var_ptr_enum, _)) = self_compiler.get_variables(name) {
                let var_ptr = var_ptr_enum.into_pointer_value();

                let current_tag = val_tag.into_int_value();

                let tag_string = self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::String as u64, false);
                let tag_list = self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::List as u64, false);
                let tag_range = self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::Range as u64, false);
                let is_string = self_compiler
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        current_tag,
                        tag_string,
                        "compile_expr_is_string",
                    )
                    .unwrap();
                let is_list = self_compiler
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        current_tag,
                        tag_list,
                        "compile_expr_is_list",
                    )
                    .unwrap();
                let is_range = self_compiler
                    .builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        current_tag,
                        tag_range,
                        "compile_expr_is_range",
                    )
                    .unwrap();

                let is_heap_1 = self_compiler
                    .builder
                    .build_or(is_string, is_list, "compile_expr_is_heap_1")
                    .unwrap();
                let should_move = self_compiler
                    .builder
                    .build_or(
                        is_heap_1,
                        self_compiler
                            .builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                is_heap_1,
                                is_range,
                                "is_heap_2",
                            )
                            .unwrap(),
                        "should_move",
                    )
                    .unwrap();

                let parent_bb = self_compiler
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();
                let move_bb = self_compiler
                    .context
                    .append_basic_block(parent_bb, "compile_expr_arg_move_bb");
                let cont_bb = self_compiler
                    .context
                    .append_basic_block(parent_bb, "compile_expr_arg_cont_bb");

                self_compiler
                    .builder
                    .build_conditional_branch(should_move, move_bb, cont_bb)
                    .unwrap();

                self_compiler.builder.position_at_end(move_bb);
                let var_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(
                        self_compiler.runtime_value_type,
                        var_ptr,
                        0,
                        "compile_expr_var_tag_ptr",
                    )
                    .unwrap();
                self_compiler
                    .builder
                    .build_store(
                        var_tag_ptr,
                        self_compiler
                            .context
                            .i32_type()
                            .const_int(Tag::Unit as u64, false),
                    )
                    .unwrap();
                self_compiler
                    .builder
                    .build_unconditional_branch(cont_bb)
                    .unwrap();

                self_compiler.builder.position_at_end(cont_bb);
            }
        }
    }
    let call_site = self_compiler
        .builder
        .build_call(func, &compiled_args, "compile_expr_call_tmp")
        .unwrap();

    let return_type_opt = func.get_type().get_return_type();
    if return_type_opt.is_none() {
        return create_unit(self_compiler);
    }
    let return_type = return_type_opt.unwrap();
    let result_val = match call_site.try_as_basic_value() {
        ValueKind::Basic(val) => val,
        ValueKind::Instruction(_) => {
            return Err("Expected basic value from function call".to_string());
        }
    };

    box_return_value(self_compiler, return_type, result_val)
}

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

    let _ = create_panic_err(self_compiler, &error_message, module, settings)?;

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
    let int_res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            int_res_ptr,
            0,
            "int_res_tag_ptr",
        )
        .unwrap();

    self_compiler
        .builder
        .build_store(int_res_tag_ptr, l_tag)
        .unwrap();

    let int_res_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            int_res_ptr,
            1,
            "int_res_data_ptr",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(int_res_data_ptr, int_sum)
        .unwrap();

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
    let float_res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            float_res_ptr,
            0,
            "float_res_tag_ptr",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(float_res_tag_ptr, float_tag)
        .unwrap();
    let float_res_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            float_res_ptr,
            1,
            "float_res_data_ptr",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(float_res_data_ptr, res_data)
        .unwrap();
    Ok(float_res_ptr)
}

fn create_add_expr_build_string_branch<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    l_ptr: PointerValue<'ctx>,
    r_ptr: PointerValue<'ctx>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
    let l_str_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_str_data_ptr")
        .unwrap();
    let l_str_ptr_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            l_str_data_ptr,
            "l_str_ptr_int",
        )
        .unwrap()
        .into_int_value();
    let l_str_ptr = self_compiler
        .builder
        .build_int_to_ptr(
            l_str_ptr_int,
            self_compiler.context.ptr_type(AddressSpace::default()),
            "l_str_ptr",
        )
        .unwrap();
    let r_str_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_str_data_ptr")
        .unwrap();
    let r_str_ptr_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            r_str_data_ptr,
            "r_str_ptr_int",
        )
        .unwrap()
        .into_int_value();
    let r_str_ptr = self_compiler
        .builder
        .build_int_to_ptr(
            r_str_ptr_int,
            self_compiler.context.ptr_type(AddressSpace::default()),
            "r_str_ptr",
        )
        .unwrap();

    let strlen_fn = self_compiler.get_runtime_fn(module, "__strlen");
    let malloc_fn = self_compiler.get_runtime_fn(module, "__malloc");

    let l_len = self_compiler
        .builder
        .build_call(strlen_fn, &[l_str_ptr.into()], "l_strlen_call")
        .unwrap();

    let l_len_val = match l_len.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => return Err("Expected basic value from strlen".to_string()),
    };

    let r_len = self_compiler
        .builder
        .build_call(strlen_fn, &[r_str_ptr.into()], "r_strlen_call")
        .unwrap();

    let r_len_val = match r_len.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_int_value(),
        _ => return Err("Expected basic value from strlen".to_string()),
    };

    let total_len = self_compiler
        .builder
        .build_int_add(l_len_val, r_len_val, "total_str_len")
        .unwrap();
    let one = self_compiler.context.i64_type().const_int(1, false); // for null terminator
    let alloc_size = self_compiler
        .builder
        .build_int_add(total_len, one, "alloc_size")
        .unwrap();

    let malloc_call = self_compiler
        .builder
        .build_call(malloc_fn, &[alloc_size.into()], "malloc_call")
        .unwrap();

    let malloc_ptr = match malloc_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_pointer_value(),
        _ => return Err("Expected basic value from malloc".to_string()),
    };

    self_compiler
        .builder
        .build_memcpy(malloc_ptr, 1, l_str_ptr, 1, l_len_val)
        .unwrap();

    let dest_ptr = unsafe {
        self_compiler
            .builder
            .build_gep(
                self_compiler.context.i8_type(),
                malloc_ptr,
                &[l_len_val],
                "dest_ptr",
            )
            .unwrap()
    };
    self_compiler
        .builder
        .build_memcpy(dest_ptr, 1, r_str_ptr, 1, r_len_val)
        .unwrap();

    let end_ptr = unsafe {
        self_compiler
            .builder
            .build_gep(
                self_compiler.context.i8_type(),
                malloc_ptr,
                &[total_len],
                "end_ptr",
            )
            .unwrap()
    };
    self_compiler
        .builder
        .build_store(end_ptr, self_compiler.context.i8_type().const_int(0, false))
        .unwrap();

    let str_res_ptr = create_entry_block_alloca(self_compiler, "str_res_alloc");

    let str_res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            str_res_ptr,
            0,
            "str_res_tag_ptr",
        )
        .unwrap();

    let check_string = self_compiler
        .context
        .i32_type()
        .const_int(Tag::String as u64, false);

    self_compiler
        .builder
        .build_store(str_res_tag_ptr, check_string)
        .unwrap();

    let str_res_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            str_res_ptr,
            1,
            "str_res_data_ptr",
        )
        .unwrap();
    let malloc_ptr_as_i64 = self_compiler
        .builder
        .build_ptr_to_int(
            malloc_ptr,
            self_compiler.context.i64_type(),
            "malloc_ptr_as_i64",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(str_res_data_ptr, malloc_ptr_as_i64)
        .unwrap();

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

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int8 as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint8 as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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
    let res_tag_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    _self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            _self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int16 as u64, false),
        )
        .unwrap();

    let res_data_ptr = _self_compiler
        .builder
        .build_struct_gep(
            _self_compiler.runtime_value_type,
            res_ptr,
            1,
            "res_data_ptr",
        )
        .unwrap();
    _self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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
    let res_tag_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    _self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            _self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint16 as u64, false),
        )
        .unwrap();

    let res_data_ptr = _self_compiler
        .builder
        .build_struct_gep(
            _self_compiler.runtime_value_type,
            res_ptr,
            1,
            "res_data_ptr",
        )
        .unwrap();
    _self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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
    let res_tag_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    _self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            _self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int32 as u64, false),
        )
        .unwrap();

    let res_data_ptr = _self_compiler
        .builder
        .build_struct_gep(
            _self_compiler.runtime_value_type,
            res_ptr,
            1,
            "res_data_ptr",
        )
        .unwrap();
    _self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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
    let res_tag_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    _self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            _self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint32 as u64, false),
        )
        .unwrap();
    let res_data_ptr = _self_compiler
        .builder
        .build_struct_gep(
            _self_compiler.runtime_value_type,
            res_ptr,
            1,
            "res_data_ptr",
        )
        .unwrap();
    _self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int64 as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, res_val)
        .unwrap();

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

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint64 as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, res_val)
        .unwrap();

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
    let res_tag_ptr = _self_compiler
        .builder
        .build_struct_gep(_self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    _self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            _self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float16 as u64, false),
        )
        .unwrap();
    let res_data_ptr = _self_compiler
        .builder
        .build_struct_gep(
            _self_compiler.runtime_value_type,
            res_ptr,
            1,
            "res_data_ptr",
        )
        .unwrap();
    _self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float32 as u64, false),
        )
        .unwrap();
    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();

    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float64 as u64, false),
        )
        .unwrap();
    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, res_i64)
        .unwrap();

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

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Integer as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, result)
        .unwrap();
    Ok(res_ptr.into())
}

pub enum UpDown {
    Up = 0,
    Down = 1,
}

pub fn create_increment_or_decrement<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    expr: &ast::Expr,
    mode: UpDown,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
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

pub fn create_eq_or_neq<'ctx, F>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
    mode: EqNeq,
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
        match mode {
            EqNeq::Eq => "eq",
            EqNeq::Neq => "neq",
        },
    )?;

    let res_ptr = create_entry_block_alloca(self_compiler, "eq_or_neq_res_alloc");

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Boolean as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    let bool_as_i64 = self_compiler
        .builder
        .build_int_z_extend(result, self_compiler.context.i64_type(), "bool_as_i64")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, bool_as_i64)
        .unwrap();

    Ok(res_ptr.into())
}

pub enum Comparison {
    Gt = 0,
    Lt = 1,
    Ge = 2,
    Le = 3,
}

pub fn create_comparison<'ctx, F>(
    self_compiler: &mut Compiler<'ctx>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
    mode: Comparison,
    comp_fn: F,
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

    let res_ptr = create_entry_block_alloca(self_compiler, "comparison_res_alloc");

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Boolean as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    let bool_as_i64 = self_compiler
        .builder
        .build_int_z_extend(result, self_compiler.context.i64_type(), "bool_as_i64")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, bool_as_i64)
        .unwrap();
    Ok(res_ptr.into())
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

pub fn create_list<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    elements: &Vec<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let list_ptr = self_compiler.build_list_from_exprs(elements, module)?;
    let i64_type = self_compiler.context.i64_type();

    let res_ptr = create_entry_block_alloca(self_compiler, "list_res_alloc");
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::List as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    let list_ptr_as_int = self_compiler
        .builder
        .build_ptr_to_int(list_ptr, i64_type, "list_ptr_as_int")
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, list_ptr_as_int)
        .unwrap();

    Ok(res_ptr.into())
}

pub fn create_index<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    collection_expr: &ast::Expr,
    index_expr: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let get_fn = self_compiler.get_runtime_fn(module, "__list_get");

    let collection_var_ptr = self_compiler
        .compile_expr(collection_expr, module)?
        .into_pointer_value();

    let list_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            collection_var_ptr,
            1,
            "list_data_ptr",
        )
        .unwrap();
    let list_ptr_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            list_data_ptr,
            "list_ptr_int",
        )
        .unwrap()
        .into_int_value();

    let list_ptr = self_compiler
        .builder
        .build_int_to_ptr(
            list_ptr_int,
            self_compiler.context.ptr_type(AddressSpace::default()),
            "list_ptr",
        )
        .unwrap();

    let index_val_ptr = self_compiler
        .compile_expr(index_expr, module)?
        .into_pointer_value();

    let index_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            index_val_ptr,
            1,
            "index_data_ptr",
        )
        .unwrap();
    let index_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            index_data_ptr,
            "index_int",
        )
        .unwrap()
        .into_int_value();

    let get_call = self_compiler
        .builder
        .build_call(
            get_fn,
            &[list_ptr.into(), index_int.into()],
            "list_get_call",
        )
        .unwrap();

    match get_call.try_as_basic_value() {
        ValueKind::Basic(val) => Ok(val),
        ValueKind::Instruction(_) => Err("Expected basic value from __list_get".to_string()),
    }
}

pub fn create_range<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    start_expr: &ast::Expr,
    end_expr: &ast::Expr,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let range_fn = self_compiler.get_runtime_fn(module, "__range_new");
    let start_val_ptr = self_compiler
        .compile_expr(start_expr, module)?
        .into_pointer_value();
    let start_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            start_val_ptr,
            1,
            "start_data_ptr",
        )
        .unwrap();
    let start_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            start_data_ptr,
            "start_int",
        )
        .unwrap()
        .into_int_value();

    let end_val_ptr = self_compiler
        .compile_expr(end_expr, module)?
        .into_pointer_value();
    let end_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            end_val_ptr,
            1,
            "end_data_ptr",
        )
        .unwrap();
    let end_int = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), end_data_ptr, "end_int")
        .unwrap()
        .into_int_value();

    let range_call = self_compiler
        .builder
        .build_call(range_fn, &[start_int.into(), end_int.into()], "range_call")
        .unwrap();
    let range_ptr = match range_call.try_as_basic_value() {
        ValueKind::Basic(val) => val.into_pointer_value(),
        ValueKind::Instruction(_) => {
            return Err("Expected basic value from __range_new".to_string());
        }
    };

    let res_ptr = create_entry_block_alloca(self_compiler, "range_res_alloc");

    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Range as u64, false),
        )
        .unwrap();

    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
        .unwrap();
    let range_ptr_as_int = self_compiler
        .builder
        .build_ptr_to_int(
            range_ptr,
            self_compiler.context.i64_type(),
            "range_ptr_as_int",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, range_ptr_as_int)
        .unwrap();
    Ok(res_ptr.into())
}

pub fn create_module_access<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    module_name: &str,
    function_name: &str,
    args: &Vec<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let target_module = self_compiler
        .modules
        .get(module_name)
        .ok_or_else(|| format!("Module '{}' not found", module_name))?;

    let target_func = target_module.get_function(&function_name).ok_or_else(|| {
        format!(
            "Function '{}' not found in module '{}'",
            function_name, module_name
        )
    })?;

    let func_in_current_module = if let Some(func) = module.get_function(&function_name) {
        func
    } else {
        module.add_function(&function_name, target_func.get_type(), None)
    };

    let mut compiled_args = Vec::with_capacity(args.len());
    for arg_expr in args {
        let arg_val = self_compiler.compile_expr(arg_expr, module)?.into();
        compiled_args.push(arg_val);
    }

    let call_site = self_compiler
        .builder
        .build_call(func_in_current_module, &compiled_args, "module_func_call")
        .unwrap();

    let return_type_opt = target_func.get_type().get_return_type();
    if return_type_opt.is_none() {
        return create_unit(self_compiler);
    }
    let return_type = return_type_opt.unwrap();

    let result_val = match call_site.try_as_basic_value() {
        ValueKind::Basic(val) => val,
        ValueKind::Instruction(_) => {
            return Err("Expected basic value from module function call".to_string());
        }
    };

    box_return_value(self_compiler, return_type, result_val)
}

pub fn create_field_access<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    struct_expr: &ast::Expr,
    field_index: u32,
    struct_name: &str,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let struct_ptr = self_compiler
        .compile_expr(struct_expr, module)?
        .into_pointer_value();

    let struct_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            struct_ptr,
            1,
            "struct_data_ptr",
        )
        .unwrap();

    let heap_ptr_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            struct_data_ptr,
            "heap_ptr_int",
        )
        .unwrap()
        .into_int_value();

    let heap_ptr = self_compiler
        .builder
        .build_int_to_ptr(
            heap_ptr_int,
            self_compiler.context.ptr_type(AddressSpace::default()),
            "heap_ptr",
        )
        .unwrap();

    let struct_def = self_compiler
        .struct_defs
        .get(struct_name)
        .ok_or_else(|| format!("Undefined struct : {}", struct_name))?;
    let llvm_type = struct_def.llvm_type;
    let field_def = &struct_def.fields[field_index as usize];

    let struct_ptr_typed = self_compiler
        .builder
        .build_pointer_cast(
            heap_ptr,
            llvm_type.get_context().ptr_type(AddressSpace::default()),
            "struct_ptr_typed",
        )
        .unwrap();

    let field_ptr = self_compiler
        .builder
        .build_struct_gep(llvm_type, struct_ptr_typed, field_index, "field_ptr")
        .unwrap();

    if let Some(ty) = &field_def.ty {
        if crate::interpreter::type_helper::is_int_type_in_llvm().contains(ty) {
            match ty {
                crate::interpreter::type_helper::Type::Int
                | crate::interpreter::type_helper::Type::TypeI64
                | crate::interpreter::type_helper::Type::TypeU64 => {
                    let val = self_compiler
                        .builder
                        .build_load(self_compiler.context.i64_type(), field_ptr, "field_val")
                        .unwrap()
                        .into_int_value();

                    let res_ptr =
                        create_entry_block_alloca(self_compiler, "int_field_access_res_alloc");
                    let res_tag_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            res_ptr,
                            0,
                            "res_tag_ptr",
                        )
                        .unwrap();
                    self_compiler
                        .builder
                        .build_store(
                            res_tag_ptr,
                            self_compiler
                                .context
                                .i32_type()
                                .const_int(Tag::Integer as u64, false),
                        )
                        .unwrap();
                    let res_data_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            res_ptr,
                            1,
                            "res_data_ptr",
                        )
                        .unwrap();
                    self_compiler
                        .builder
                        .build_store(res_data_ptr, val)
                        .unwrap();
                    return Ok(res_ptr.into());
                }
                crate::interpreter::type_helper::Type::Str => {
                    let val = self_compiler
                        .builder
                        .build_load(
                            self_compiler.context.ptr_type(AddressSpace::default()),
                            field_ptr,
                            "str_field_ptr_load",
                        )
                        .unwrap()
                        .into_pointer_value();
                    let var_int = self_compiler
                        .builder
                        .build_ptr_to_int(
                            val,
                            self_compiler.context.i64_type(),
                            "str_field_ptr_as_int",
                        )
                        .unwrap();
                    let res_ptr =
                        create_entry_block_alloca(self_compiler, "str_field_access_res_alloc");
                    let tag_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            res_ptr,
                            0,
                            "res_tag_ptr",
                        )
                        .unwrap();
                    self_compiler
                        .builder
                        .build_store(
                            tag_ptr,
                            self_compiler
                                .context
                                .i32_type()
                                .const_int(Tag::String as u64, false),
                        )
                        .unwrap();
                    let data_ptr = self_compiler
                        .builder
                        .build_struct_gep(
                            self_compiler.runtime_value_type,
                            res_ptr,
                            1,
                            "res_data_ptr",
                        )
                        .unwrap();
                    self_compiler
                        .builder
                        .build_store(data_ptr, var_int)
                        .unwrap();
                    return Ok(res_ptr.into());
                }
                _ => { /* Fallback to generic field access */ }
            }
        }
    }

    let field_val = self_compiler
        .builder
        .build_load(self_compiler.runtime_value_type, field_ptr, "field_val")
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "field_access_res_alloc");

    self_compiler
        .builder
        .build_store(res_ptr, field_val)
        .unwrap();

    Ok(res_ptr.into())
}

pub fn create_unit<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let res_ptr = create_entry_block_alloca(self_compiler, "unit_res_alloc");
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Unit as u64, false),
        )
        .unwrap();
    Ok(res_ptr.into())
}

pub fn create_struct_init<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    struct_name: &str,
    field_exprs: &[(String, ast::Expr)],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let struct_def = self_compiler
        .struct_defs
        .get(struct_name)
        .ok_or_else(|| format!("Undefined struct : {}", struct_name))?;

    let llvm_type = struct_def.llvm_type;
    let field_indices = struct_def.field_indices.clone();
    let def_fields = struct_def.fields.clone();

    let struct_ptr = self_compiler
        .builder
        .build_malloc(llvm_type, &format!("{}_struct_alloc", struct_name))
        .map_err(|e| e.to_string())?;

    for (field_name, field_expr) in field_exprs {
        let index = field_indices.get(field_name).ok_or_else(|| {
            format!(
                "Field '{}' not found in struct '{}'",
                field_name, struct_name
            )
        })?;

        let field_def = def_fields
            .iter()
            .find(|f| f.ident == *field_name)
            .ok_or_else(|| {
                format!(
                    "Field definition for '{}' not found in struct '{}'",
                    field_name, struct_name
                )
            })?;

        let value = self_compiler.compile_expr(field_expr, module)?;

        let field_ptr = self_compiler
            .builder
            .build_struct_gep(llvm_type, struct_ptr, *index, "field_ptr")
            .map_err(|e| e.to_string())?;

        if let Some(ty) = &field_def.ty {
            if crate::interpreter::type_helper::is_int_type_in_llvm().contains(ty) {
                match ty {
                    crate::interpreter::type_helper::Type::Int
                    | crate::interpreter::type_helper::Type::TypeI64
                    | crate::interpreter::type_helper::Type::TypeU64 => {
                        let val_ptr = value.into_pointer_value();
                        let data_ptr = self_compiler
                            .builder
                            .build_struct_gep(
                                self_compiler.runtime_value_type,
                                val_ptr,
                                1,
                                "int_field_data_ptr",
                            )
                            .unwrap();
                        let int_val = self_compiler
                            .builder
                            .build_load(self_compiler.context.i64_type(), data_ptr, "int_field_val")
                            .unwrap()
                            .into_int_value();
                        self_compiler
                            .builder
                            .build_store(field_ptr, int_val)
                            .unwrap();
                        continue;
                    }
                    crate::interpreter::type_helper::Type::Str => {
                        let val_ptr = value.into_pointer_value();
                        let data_ptr = self_compiler
                            .builder
                            .build_struct_gep(
                                self_compiler.runtime_value_type,
                                val_ptr,
                                1,
                                "str_field_data_ptr",
                            )
                            .unwrap();
                        let str_ptr_int = self_compiler
                            .builder
                            .build_load(
                                self_compiler.context.i64_type(),
                                data_ptr,
                                "str_field_ptr_int",
                            )
                            .unwrap()
                            .into_int_value();
                        let str_ptr = self_compiler
                            .builder
                            .build_int_to_ptr(
                                str_ptr_int,
                                self_compiler.context.ptr_type(AddressSpace::default()),
                                "str_field_ptr",
                            )
                            .unwrap();
                        self_compiler
                            .builder
                            .build_store(field_ptr, str_ptr)
                            .unwrap();
                        continue;
                    }
                    _ => { /* Fallback to generic field store */ }
                }
            }
        }

        let val_to_store = if value.is_pointer_value() {
            self_compiler
                .builder
                .build_load(
                    self_compiler.runtime_value_type,
                    value.into_pointer_value(),
                    "field_value",
                )
                .unwrap()
        } else {
            value
        };
        self_compiler
            .builder
            .build_store(field_ptr, val_to_store)
            .unwrap();
    }

    let allloca = self_compiler
        .builder
        .build_alloca(self_compiler.runtime_value_type, "struct_init_res_alloc")
        .unwrap();

    let tag = self_compiler
        .context
        .i32_type()
        .const_int(Tag::Struct as u64, false);
    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, allloca, 0, "tag_ptr")
        .unwrap();
    self_compiler.builder.build_store(tag_ptr, tag).unwrap();

    let data_int = self_compiler
        .builder
        .build_ptr_to_int(
            struct_ptr,
            self_compiler.context.i64_type(),
            "struct_ptr_as_int",
        )
        .unwrap();
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, allloca, 1, "data_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(data_ptr, data_int)
        .unwrap();

    Ok(allloca.into())
}

// !Define builtin macro handlers

pub fn call_builtin_macro_println<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let print_fn = self_compiler.get_runtime_fn(module, "__println");

    let list_ptr = self_compiler.build_list_from_exprs(args, module)?;

    self_compiler
        .builder
        .build_call(print_fn, &[list_ptr.into()], "println_call")
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "println_res_alloc");
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Unit as u64, false),
        )
        .unwrap();

    return Ok(res_ptr.into());
}

pub fn call_builtin_macro_list_push<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    if args.len() != 2 {
        return Err("list_push expects 2 arguments".to_string());
    }
    let list_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let val_ptr = self_compiler
        .compile_expr(&args[1], module)?
        .into_pointer_value();

    let list_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            list_ptr,
            1,
            "list_data_ptr",
        )
        .unwrap();
    let list_vec_int = self_compiler
        .builder
        .build_load(
            self_compiler.context.i64_type(),
            list_data_ptr,
            "list_vec_int",
        )
        .unwrap()
        .into_int_value();
    let list_vec_ptr = self_compiler
        .builder
        .build_int_to_ptr(
            list_vec_int,
            self_compiler.context.ptr_type(AddressSpace::default()),
            "list_vec_ptr",
        )
        .unwrap();

    let target_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 0, "val_tag_ptr")
        .unwrap();
    let val_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), target_ptr, "val_tag")
        .unwrap()
        .into_int_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 1, "val_data_ptr")
        .unwrap();
    let val_data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "val_data")
        .unwrap()
        .into_int_value();

    let list_push_fn = self_compiler.get_runtime_fn(module, "__list_push");
    self_compiler
        .builder
        .build_call(
            list_push_fn,
            &[list_vec_ptr.into(), val_tag.into(), val_data.into()],
            "list_push_call",
        )
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "list_push_res_alloc");
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
        .unwrap();
    self_compiler
        .builder
        .build_store(
            res_tag_ptr,
            self_compiler
                .context
                .i32_type()
                .const_int(Tag::Unit as u64, false),
        )
        .unwrap();

    return Ok(res_ptr.into());
}

pub fn call_builtin_macro_clone<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    if args.len() != 1 {
        return Err("clone! expects 1 argument".to_string());
    }
    let arg_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            arg_ptr,
            0,
            "clone_arg_tag_ptr",
        )
        .unwrap();
    let tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), tag_ptr, "clone_arg_tag")
        .unwrap()
        .into_int_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            arg_ptr,
            1,
            "clone_arg_data_ptr",
        )
        .unwrap();
    let data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "clone_arg_data")
        .unwrap()
        .into_int_value();

    let clone_fn = self_compiler.get_runtime_fn(module, "__clone");
    let call_site = self_compiler
        .builder
        .build_call(clone_fn, &[tag.into(), data.into()], "clone_call")
        .unwrap();
    let result_val = match call_site.try_as_basic_value() {
        ValueKind::Basic(val) => Ok(val),
        ValueKind::Instruction(_) => Err("Expected basic value from clone function".to_string()),
    };

    let result_ptr = create_entry_block_alloca(self_compiler, "clone_res_alloc");

    self_compiler
        .builder
        .build_store(result_ptr, result_val?)
        .unwrap();

    return Ok(result_ptr.into());
}

pub fn call_builtin_macro_cast<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &Vec<ast::Expr>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    if args.len() != 2 {
        return Err("cast! expects 2 arguments".to_string());
    }

    let value_ptr = self_compiler
        .compile_expr(&args[0], module)?
        .into_pointer_value();
    let target_type_expr = &args[1];

    let target_type = match target_type_expr {
        ast::Expr::Var(ident) => ident.as_str(),
        ast::Expr::TypeI8 => "i8",
        ast::Expr::TypeU8 => "u8",
        ast::Expr::TypeI16 => "i16",
        ast::Expr::TypeU16 => "u16",
        ast::Expr::TypeI32 => "i32",
        ast::Expr::TypeU32 => "u32",
        ast::Expr::TypeI64 => "i64",
        ast::Expr::TypeU64 => "u64",

        ast::Expr::TypeF16 => "fp16",
        ast::Expr::TypeF32 => "fp32",
        ast::Expr::TypeF64 => "fp64",
        _ => {
            return Err(format!(
                "cast! second argument must be a type identifier : {:?}",
                target_type_expr
            ));
        }
    };

    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            value_ptr,
            0,
            "cast_arg_tag_ptr",
        )
        .unwrap();

    // Load the current tag (not used here but could be useful for type checking)
    let current_tag = self_compiler
        .builder
        .build_load(self_compiler.context.i32_type(), tag_ptr, "cast_arg_tag")
        .unwrap()
        .into_int_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            value_ptr,
            1,
            "cast_arg_data_ptr",
        )
        .unwrap();
    let data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "cast_arg_data")
        .unwrap()
        .into_int_value();

    let parent = self_compiler
        .builder
        .get_insert_block()
        .unwrap()
        .get_parent()
        .unwrap();

    let bb_int = self_compiler
        .context
        .append_basic_block(parent, "cast_int_bb");
    let bb_float = self_compiler
        .context
        .append_basic_block(parent, "cast_float_bb");
    let bb_f16 = self_compiler
        .context
        .append_basic_block(parent, "cast_f16_bb");
    let bb_f32 = self_compiler
        .context
        .append_basic_block(parent, "cast_f32_bb");
    let bb_f64 = self_compiler
        .context
        .append_basic_block(parent, "cast_f64_bb");
    let marge = self_compiler
        .context
        .append_basic_block(parent, "cast_merge_bb");

    let i32_type = self_compiler.context.i32_type();
    let cases = vec![
        (i32_type.const_int(Tag::Integer as u64, false), bb_int),
        (i32_type.const_int(Tag::Float as u64, false), bb_float),
        (i32_type.const_int(Tag::Float16 as u64, false), bb_f16),
        (i32_type.const_int(Tag::Float32 as u64, false), bb_f32),
        (i32_type.const_int(Tag::Float64 as u64, false), bb_f64),
    ];

    self_compiler
        .builder
        .build_switch(current_tag, bb_f64, &cases)
        .unwrap();

    // Integer -> f64
    self_compiler.builder.position_at_end(bb_int);
    let int_to_f64 = self_compiler
        .builder
        .build_signed_int_to_float(data, self_compiler.context.f64_type(), "int_to_f64")
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float -> f64
    self_compiler.builder.position_at_end(bb_float);
    let float_to_f64 = self_compiler
        .builder
        .build_bit_cast(data, self_compiler.context.f64_type(), "float_to_f64")
        .unwrap()
        .into_float_value();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float16 -> f64
    self_compiler.builder.position_at_end(bb_f16);
    let f16_to_f64 = self_compiler
        .builder
        .build_int_truncate(data, self_compiler.context.i16_type(), "f16_to_f64")
        .unwrap();
    let val_f16 = self_compiler
        .builder
        .build_bit_cast(
            f16_to_f64,
            self_compiler.context.f16_type(),
            "f16_to_f64_cast",
        )
        .unwrap()
        .into_float_value();

    let val_f16_ext = self_compiler
        .builder
        .build_float_ext(val_f16, self_compiler.context.f64_type(), "f16_to_f64_ext")
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float32 -> f64
    self_compiler.builder.position_at_end(bb_f32);
    let val_f32_i32 = self_compiler
        .builder
        .build_int_truncate(data, self_compiler.context.i32_type(), "f32_to_f64")
        .unwrap();
    let val_f32 = self_compiler
        .builder
        .build_bit_cast(
            val_f32_i32,
            self_compiler.context.f32_type(),
            "f32_to_f64_cast",
        )
        .unwrap()
        .into_float_value();
    let val_f32_ext = self_compiler
        .builder
        .build_float_ext(val_f32, self_compiler.context.f64_type(), "f32_to_f64_ext")
        .unwrap();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Float64 -> f64
    self_compiler.builder.position_at_end(bb_f64);
    let val_f64 = self_compiler
        .builder
        .build_bit_cast(data, self_compiler.context.f64_type(), "f64_to_f64")
        .unwrap()
        .into_float_value();
    self_compiler
        .builder
        .build_unconditional_branch(marge)
        .unwrap();

    // Merge block
    self_compiler.builder.position_at_end(marge);
    let phi = self_compiler
        .builder
        .build_phi(self_compiler.context.f64_type(), "cast_phi")
        .unwrap();
    phi.add_incoming(&[
        (&int_to_f64, bb_int),
        (&float_to_f64, bb_float),
        (&val_f16_ext, bb_f16),
        (&val_f32_ext, bb_f32),
        (&val_f64, bb_f64),
    ]);
    let normalized_f64 = phi.as_basic_value().into_float_value();

    let (new_tag, new_data) = match target_type {
        "i8" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int8 as u64, false);

            let new_data = self_compiler
                .builder
                .build_int_truncate(data, self_compiler.context.i8_type(), "cast_to_int8")
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_s_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_int8_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "u8" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint8 as u64, false);

            let new_data = self_compiler
                .builder
                .build_int_truncate(data, self_compiler.context.i8_type(), "cast_to_uint8")
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_uint8_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "i16" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int16 as u64, false);

            let new_data = self_compiler
                .builder
                .build_int_truncate(data, self_compiler.context.i16_type(), "cast_to_int16")
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_s_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_int16_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "u16" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint16 as u64, false);

            let new_data = self_compiler
                .builder
                .build_int_truncate(data, self_compiler.context.i16_type(), "cast_to_uint16")
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_uint16_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "i32" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int32 as u64, false);

            let new_data = self_compiler
                .builder
                .build_int_truncate(data, self_compiler.context.i32_type(), "cast_to_int32")
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_s_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_int32_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "u32" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint32 as u64, false);

            let new_data = self_compiler
                .builder
                .build_int_truncate(data, self_compiler.context.i32_type(), "cast_to_uint32")
                .unwrap();
            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data,
                    self_compiler.context.i64_type(),
                    "cast_to_uint32_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }
        "i64" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Int64 as u64, false);
            (new_tag, data)
        }
        "u64" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Uint64 as u64, false);
            (new_tag, data)
        }

        "fp16" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float16 as u64, false);

            // f64 -> f16

            let new_data = self_compiler
                .builder
                .build_float_trunc(
                    normalized_f64,
                    self_compiler.context.f16_type(),
                    "cast_to_fp16",
                )
                .unwrap();

            let new_data_i16 = self_compiler
                .builder
                .build_bit_cast(new_data, self_compiler.context.i16_type(), "fp16_to_i16")
                .unwrap()
                .into_int_value();

            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data_i16,
                    self_compiler.context.i64_type(),
                    "cast_to_fp16_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }

        "fp32" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float32 as u64, false);

            // f64 -> f32

            let new_data = self_compiler
                .builder
                .build_float_trunc(
                    normalized_f64,
                    self_compiler.context.f32_type(),
                    "cast_to_fp32",
                )
                .unwrap();

            let new_data_i32 = self_compiler
                .builder
                .build_bit_cast(new_data, self_compiler.context.i32_type(), "fp32_to_i32")
                .unwrap()
                .into_int_value();

            let new_data_ext = self_compiler
                .builder
                .build_int_z_extend(
                    new_data_i32,
                    self_compiler.context.i64_type(),
                    "cast_to_fp32_ext",
                )
                .unwrap();
            (new_tag, new_data_ext)
        }

        "fp64" => {
            let new_tag = self_compiler
                .context
                .i32_type()
                .const_int(Tag::Float64 as u64, false);

            let new_data = self_compiler
                .builder
                .build_bit_cast(
                    normalized_f64,
                    self_compiler.context.i64_type(),
                    "cast_to_fp64_ext",
                )
                .unwrap()
                .into_int_value();
            (new_tag, new_data)
        }
        _ => {
            return Err(format!(
                "Unsupported target type for cast!: {:?}",
                target_type
            ));
        }
    };

    let result_ptr = create_entry_block_alloca(self_compiler, "cast_res_alloc");
    let res_tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            result_ptr,
            0,
            "res_tag_ptr",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(res_tag_ptr, new_tag)
        .unwrap();
    let res_data_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            result_ptr,
            1,
            "res_data_ptr",
        )
        .unwrap();
    self_compiler
        .builder
        .build_store(res_data_ptr, new_data)
        .unwrap();
    return Ok(result_ptr.into());
}

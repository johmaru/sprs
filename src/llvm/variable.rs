use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};

use crate::llvm::compiler::{Compiler, Tag};

use crate::llvm::value::create_entry_block_alloca;
use crate::llvm::builder_helper::{BuilderExt, ContextExt};

// A runtime move system for variables that hold heap data (strings, lists, ranges)
// When passing such variables to functions, we need to "move" them by resetting their tag to Unit
// If want to keep the data, can use "clone" macro.
pub fn move_variable<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    src_enum_ptr: &BasicValueEnum<'ctx>,
    name: &str,
) {
    let src_ptr = src_enum_ptr.into_pointer_value();

    let tag_ptr = self_compiler.build_tag_gep(src_ptr, name);

    let current_tag = self_compiler.build_load_tag(tag_ptr, name);

    let tag_string = self_compiler.get_tag_from_tag_enum(Tag::String);
    let tag_list = self_compiler.get_tag_from_tag_enum(Tag::List);
    let tag_range = self_compiler.get_tag_from_tag_enum(Tag::Range);
    let tag_struct = self_compiler.get_tag_from_tag_enum(Tag::Struct);
    let tag_enum = self_compiler.get_tag_from_tag_enum(Tag::Enum);
    let is_string = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_string, name);
    let is_list = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_list, name);
    let is_range = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_range, name);
    let is_struct = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_struct, name);
    let is_enum = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_enum, name);

    // With slab, all heap tags (String/List/Range/Struct/Enum) carry a slot
    // handle in `data` and must be moved (tag reset to Unit) so the original
    // binding doesn't release the slot a second time on scope exit.
    let is_heap_1 = self_compiler.or(is_string, is_list, name);
    let is_heap_2 = self_compiler.or(is_heap_1, is_range, name);
    let is_heap_3 = self_compiler.or(is_heap_2, is_struct, name);
    let should_move = self_compiler.or(is_heap_3, is_enum, name);
    let parent_bb = self_compiler.get_current_function();
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
    self_compiler.build_tag_store(Tag::Unit, tag_ptr);
    self_compiler
        .builder
        .build_unconditional_branch(cont_bb)
        .unwrap();
    self_compiler.builder.position_at_end(cont_bb);
}

pub fn var_load_at_init_variable<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    init_value: PointerValue<'ctx>,
    name: &str,
) -> Result<PointerValue<'ctx>, String> {
    let ptr = create_entry_block_alloca(self_compiler, name)?;

    let val = self_compiler.build_load(self_compiler.runtime_value_type, init_value, name);
    let _ = self_compiler.builder.build_store(ptr, val).unwrap();
    Ok(ptr)
}

pub fn var_return_store<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    value_enum: &BasicValueEnum<'ctx>,
    name: &str,
) {
    let var_ptr = value_enum.into_pointer_value();

    self_compiler.tag_only_runtime_value_store(var_ptr, Tag::Unit as u64, name);
}

pub fn drop_var<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    ptr: PointerValue<'ctx>,
    drop_fn: FunctionValue<'_>,
    name: &str,
) {
    self_compiler.build_sprs_value_call_func(ptr, drop_fn, name, &[], false);
}

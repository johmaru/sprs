use crate::front::error::SprsError;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue, ValueKind};

use crate::llvm::compiler::{Compiler, Tag};

use crate::llvm::builder_helper::{BuilderExt, ContextExt};
use crate::llvm::value::create_entry_block_alloca;

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
    let tag_label = self_compiler.get_tag_from_tag_enum(Tag::Label);
    let tag_buffer = self_compiler.get_tag_from_tag_enum(Tag::Buffer);
    let is_string = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_string, name);
    let is_list = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_list, name);
    let is_range = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_range, name);
    let is_struct = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_struct, name);
    let is_label = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_label, name);
    let is_buffer = self_compiler.tag_cmp(inkwell::IntPredicate::EQ, current_tag, tag_buffer, name);
    // With slab, all heap tags (String/List/Range/Struct/Label/Buffer)
    // carry a slot handle in `data` and must be moved (tag reset to Unit) so
    // the original binding doesn't release the slot a second time on scope exit.
    let is_heap_1 = self_compiler.or(is_string, is_list, name);
    let is_heap_2 = self_compiler.or(is_heap_1, is_range, name);
    let is_heap_3 = self_compiler.or(is_heap_2, is_struct, name);
    let is_heap_4 = self_compiler.or(is_heap_3, is_label, name);
    let should_move = self_compiler.or(is_heap_4, is_buffer, name);
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

pub fn clone_runtime_value<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    src_ptr: PointerValue<'ctx>,
    module: &inkwell::module::Module<'ctx>,
) -> Result<PointerValue<'ctx>, SprsError> {
    let tag_ptr = self_compiler
        .builder
        .build_struct_gep(
            self_compiler.runtime_value_type,
            src_ptr,
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
            src_ptr,
            1,
            "clone_arg_data_ptr",
        )
        .unwrap();
    let data = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "clone_arg_data")
        .unwrap()
        .into_int_value();

    let clone_fn = self_compiler.get_runtime_fn(module, "__clone")?;
    let call_site = self_compiler
        .builder
        .build_call(clone_fn, &[tag.into(), data.into()], "clone_call")
        .unwrap();
    let result_val = match call_site.try_as_basic_value() {
        ValueKind::Basic(val) => Ok(val),
        ValueKind::Instruction(_) => Err(SprsError::Internal {
            message: "Expected basic value from clone function".to_string(),
            location: None,
        }),
    };

    let result_ptr = create_entry_block_alloca(self_compiler, "clone_res_alloc")?;

    self_compiler
        .builder
        .build_store(result_ptr, result_val?)
        .unwrap();

    Ok(result_ptr.into())
}

pub fn var_load_at_init_variable<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    init_value: PointerValue<'ctx>,
    name: &str,
) -> Result<PointerValue<'ctx>, SprsError> {
    let ptr = create_entry_block_alloca(self_compiler, name)?;

    let val = self_compiler.build_load(self_compiler.runtime_value_type, init_value, name);
    let _ = self_compiler.builder.build_store(ptr, val).unwrap();
    Ok(ptr)
}

#[allow(dead_code)]
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

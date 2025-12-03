use inkwell::{AddressSpace, builder::Builder, module::Linkage, values::{BasicValueEnum, FunctionValue, PointerValue, ValueKind}};

use crate::{front::ast, llvm::compiler::{Compiler, Tag}};

pub fn create_list_from_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, elements: &[ast::Expr], module: &inkwell::module::Module<'ctx>) -> Result<PointerValue<'ctx>, String> {
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
            let val_ptr = self_compiler.compile_expr(elem, module)?.into_pointer_value();

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

            self_compiler.builder
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
pub fn move_variable<'ctx>(self_compiler: &mut Compiler<'ctx>, src_enum_ptr: &BasicValueEnum<'ctx>, name: &str) {
    let src_ptr = src_enum_ptr.into_pointer_value();

                                let tag_ptr = self_compiler.builder
                                    .build_struct_gep(
                                        self_compiler.runtime_value_type,
                                        src_ptr,
                                        0,
                                        &format!("{}_tag_ptr", name)
                                    )
                                    .unwrap();

                                let current_tag = self_compiler
                                    .builder
                                    .build_load(self_compiler.context.i32_type(), tag_ptr, &format!("{}_current_tag", name))
                                    .unwrap()
                                    .into_int_value();

                                let tag_string =
                                    self_compiler.context.i32_type().const_int(Tag::String as u64, false);
                                let tag_list =
                                    self_compiler.context.i32_type().const_int(Tag::List as u64, false);
                                let tag_range =
                                    self_compiler.context.i32_type().const_int(Tag::Range as u64, false);
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
                                    .build_or(
                                        is_heap_1,
                                        is_range,
                                        &format!("{}_should_move", name),
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
                                    .append_basic_block(parent_bb, &format!("{}_move_bb", name));
                                let cont_bb = self_compiler
                                    .context
                                    .append_basic_block(parent_bb, &format!("{}_cont_bb", name));

                                let _ = self_compiler.builder.build_conditional_branch(
                                    should_move,
                                    move_bb,
                                    cont_bb,
                                );

                                self_compiler.builder.position_at_end(move_bb);
                                self_compiler.builder
                                    .build_store(
                                        tag_ptr,
                                        self_compiler.context.i32_type().const_int(Tag::Unit as u64, false),
                                    )
                                    .unwrap();
                                self_compiler.builder.build_unconditional_branch(cont_bb).unwrap();
                                self_compiler.builder.position_at_end(cont_bb);
}

pub fn load_at_init_variable_with_existing<'ctx>(self_compiler: &mut Compiler<'ctx>, init_value: PointerValue<'ctx>, ptr: PointerValue<'ctx>, name: &str) {
    let val = self_compiler
                            .builder
                            .build_load(self_compiler.runtime_value_type, init_value, &format!("{}_assign_load", name))
                            .unwrap();
                        let _ = self_compiler.builder.build_store(ptr, val);
}

pub fn var_load_at_init_variable<'ctx>(self_compiler: &mut Compiler<'ctx>, init_value: PointerValue<'ctx>, name: &str) -> PointerValue<'ctx> {
    let ptr = self_compiler
                            .builder
                            .build_alloca(self_compiler.runtime_value_type, name)
                            .unwrap();

                        let val = self_compiler
                            .builder
                            .build_load(self_compiler.runtime_value_type, init_value, &format!("{}_var_load", name))
                            .unwrap();
                        let _ = self_compiler.builder.build_store(ptr, val).unwrap();
    ptr
}

pub fn var_return_store<'ctx>(self_compiler: &mut Compiler<'ctx>, value_enum: &BasicValueEnum<'ctx>, name: &str) {
   let var_ptr = value_enum.into_pointer_value();

                                let tag_ptr = self_compiler
                                    .builder
                                    .build_struct_gep(
                                        self_compiler.runtime_value_type,
                                        var_ptr,
                                        0,
                                        &format!("{}_tag_ptr", name)
                                    )
                                    .unwrap();

                                self_compiler.builder
                                    .build_store(
                                        tag_ptr,
                                        self_compiler.context.i32_type().const_int(Tag::Unit as u64, false),
                                    )
                                    .unwrap();
}

pub fn drop_var<'ctx>(self_compiler: &mut Compiler<'ctx>, ptr: PointerValue<'ctx>, drop_fn: FunctionValue<'_>, name: &str) {
    let tag_ptr = self_compiler
                                    .builder
                                    .build_struct_gep(
                                        self_compiler.runtime_value_type,
                                        ptr,
                                        0,
                                        "var_tag_ptr",
                                    )
                                    .unwrap();
                                let tag = self_compiler
                                    .builder
                                    .build_load(self_compiler.context.i32_type(), tag_ptr, "var_tag")
                                    .unwrap()
                                    .into_int_value();

                                let data_ptr = self_compiler
                                    .builder
                                    .build_struct_gep(
                                        self_compiler.runtime_value_type,
                                        ptr,
                                        1,
                                        "var_data_ptr",
                                    )
                                    .unwrap();
                                let data = self_compiler
                                    .builder
                                    .build_load(self_compiler.context.i64_type(), data_ptr, "var_data")
                                    .unwrap()
                                    .into_int_value();

                                self_compiler.builder
                                    .build_call(
                                        drop_fn,
                                        &[tag.into(), data.into()],
                                        "drop_var_call",
                                    )
                                    .unwrap();
}

pub fn create_dummy_for_no_return<'ctx>(self_compiler: &mut Compiler<'ctx>) {
    let dummy = self_compiler
                            .builder
                            .build_alloca(self_compiler.runtime_value_type, "ret_dummy")
                            .unwrap();
                        let tag_ptr = self_compiler
                            .builder
                            .build_struct_gep(self_compiler.runtime_value_type, dummy, 0, "ret_dummy_tag")
                            .unwrap();
                        self_compiler.builder
                            .build_store(
                                tag_ptr,
                                self_compiler.context.i32_type().const_int(Tag::Unit as u64, false),
                            )
                            .unwrap();
                        let data_ptr = self_compiler
                            .builder
                            .build_struct_gep(self_compiler.runtime_value_type, dummy, 1, "ret_dummy_data")
                            .unwrap();
                        self_compiler.builder
                            .build_store(data_ptr, self_compiler.context.i64_type().const_int(0, false))
                            .unwrap();

                        let val = self_compiler
                            .builder
                            .build_load(self_compiler.runtime_value_type, dummy, "ret_dummy_val")
                            .unwrap();
                        self_compiler.builder.build_return(Some(&val)).unwrap();
}

pub fn create_if_condition<'ctx>(self_compiler: &mut Compiler<'ctx>, cond: &ast::Expr, then_blk: &Vec<ast::Stmt>, else_blk: &Option<Vec<ast::Stmt>>, module: &inkwell::module::Module<'ctx>) -> Result<(), Box<dyn std::error::Error>> {
let parent_fn = self_compiler
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_parent()
                        .unwrap();

                    let then_bb = self_compiler.context.append_basic_block(parent_fn, "then_bb");
                    let else_bb = self_compiler.context.append_basic_block(parent_fn, "else_bb");
                    let merge_bb = self_compiler.context.append_basic_block(parent_fn, "if_merge");

                    let cond_val = self_compiler.compile_expr(cond, module)?;
                    let cond_ptr = cond_val.into_pointer_value();
                    let cond_data_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, cond_ptr, 1, "cond_data_ptr")
                        .unwrap();
                    let cond_loaded = self_compiler
                        .builder
                        .build_load(self_compiler.context.i64_type(), cond_data_ptr, "cond_loaded")
                        .unwrap()
                        .into_int_value();
                    let zero = self_compiler.context.i64_type().const_int(0, false);
                    let cond_bool = self_compiler
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::NE,
                            cond_loaded,
                            zero,
                            "if_cond_bool",
                        )
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

pub fn create_while_condition<'ctx>(self_compiler: &mut Compiler<'ctx>, cond: &ast::Expr, body: &Vec<ast::Stmt>, module: &inkwell::module::Module<'ctx>) -> Result<(), Box<dyn std::error::Error>> {
    let parent_fn = self_compiler
                        .builder
                        .get_insert_block()
                        .unwrap()
                        .get_parent()
                        .unwrap();

                    let cond_bb = self_compiler.context.append_basic_block(parent_fn, "while_cond");
                    let body_bb = self_compiler.context.append_basic_block(parent_fn, "while_body");
                    let after_bb = self_compiler.context.append_basic_block(parent_fn, "while_after");

                    let _ = self_compiler.builder.build_unconditional_branch(cond_bb);
                    self_compiler.builder.position_at_end(cond_bb);
                    let cond_val = self_compiler.compile_expr(cond, module)?;
                    let cond_ptr = cond_val.into_pointer_value();

                    let cond_data_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, cond_ptr, 1, "cond_data_ptr")
                        .unwrap();
                    let cond_loaded = self_compiler
                        .builder
                        .build_load(self_compiler.context.i64_type(), cond_data_ptr, "cond_loaded")
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

pub fn create_integer<'ctx>(self_compiler: &mut Compiler<'ctx>, n: &i64) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "num_alloc")
                    .unwrap();

                let tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        tag_ptr,
                        self_compiler.context
                            .i32_type()
                            .const_int(Tag::Integer as u64, false),
                    )
                    .unwrap();

                let data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "data_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        data_ptr,
                        self_compiler.context.i64_type().const_int(*n as u64, false),
                    )
                    .unwrap();

                Ok(ptr.into())
}

pub fn create_string<'ctx>(self_compiler: &mut Compiler<'ctx>, str: &String, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
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

                let ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "str_alloc")
                    .unwrap();

                let tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(tag_ptr, self_compiler.context.i32_type().const_int(1, false))
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
                self_compiler.builder.build_store(data_ptr, str_ptr_as_i64).unwrap();

                Ok(ptr.into())
}

pub fn create_bool<'ctx>(self_compiler: &mut Compiler<'ctx>, boolean: &bool) -> Result<BasicValueEnum<'ctx>, String> {
    let ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "bool_alloc")
                    .unwrap();

                let tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, ptr, 0, "bool_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        tag_ptr,
                        self_compiler.context
                            .i32_type()
                            .const_int(Tag::Boolean as u64, false),
                    )
                    .unwrap();

                let data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, ptr, 1, "bool_data_ptr")
                    .unwrap();
                let bool_val = if *boolean { 1 } else { 0 };
                self_compiler.builder
                    .build_store(data_ptr, self_compiler.context.i64_type().const_int(bool_val, false))
                    .unwrap();

                Ok(ptr.into())
}

pub fn create_call_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, ident: &str, args: &Vec<ast::Expr>, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let func = module
                    .get_function(ident)
                    .or_else(|| self_compiler.modules.values().find_map(|m| m.get_function(ident)))
                    .ok_or(format!("Undefined function: {}", ident))?;
                let mut compiled_args = Vec::with_capacity(args.len());
                for arg in args {
                    let arg_val = self_compiler.compile_expr(arg, module)?;
                    let arg_ptr = arg_val.into_pointer_value();

                    let temp_arg_ptr = self_compiler
                        .builder
                        .build_alloca(self_compiler.runtime_value_type, "arg_tmp_alloc")
                        .unwrap();
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
                        .build_struct_gep(self_compiler.runtime_value_type, temp_arg_ptr, 0, "temp_tag_ptr")
                        .unwrap();
                    let temp_data_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, temp_arg_ptr, 1, "temp_data_ptr")
                        .unwrap();
                    self_compiler.builder.build_store(temp_tag_ptr, val_tag).unwrap();
                    self_compiler.builder.build_store(temp_data_ptr, val_data).unwrap();
                    compiled_args.push(temp_arg_ptr.into());

                    if let ast::Expr::Var(name) = arg {
                        if let Some(var_ptr_enum) = self_compiler.variables.get(name) {
                            let var_ptr = var_ptr_enum.into_pointer_value();

                            let current_tag = val_tag.into_int_value();

                            let tag_string =
                                self_compiler.context.i32_type().const_int(Tag::String as u64, false);
                            let tag_list =
                                self_compiler.context.i32_type().const_int(Tag::List as u64, false);
                            let tag_range =
                                self_compiler.context.i32_type().const_int(Tag::Range as u64, false);
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
                                    self_compiler.builder
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

                            self_compiler.builder
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
                            self_compiler.builder
                                .build_store(
                                    var_tag_ptr,
                                    self_compiler.context.i32_type().const_int(Tag::Unit as u64, false),
                                )
                                .unwrap();
                            self_compiler.builder.build_unconditional_branch(cont_bb).unwrap();

                            self_compiler.builder.position_at_end(cont_bb);
                        }
                    }
                }
                let call_site = self_compiler
                    .builder
                    .build_call(func, &compiled_args, "compile_expr_call_tmp")
                    .unwrap();

                let result_val = match call_site.try_as_basic_value() {
                    ValueKind::Basic(val) => Ok(val),
                    ValueKind::Instruction(_) => {
                        Err("Expected basic value from function call".to_string())
                    }
                };
                let result_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "compile_expr_call_res_alloc")
                    .unwrap();
                self_compiler.builder.build_store(result_ptr, result_val?).unwrap();
                Ok(result_ptr.into())
}

pub fn create_add_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let l_ptr = self_compiler.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self_compiler.compile_expr(rhs, module)?.into_pointer_value();

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
                let int_tag = self_compiler
                    .context
                    .i32_type()
                    .const_int(Tag::Integer as u64, false);
                let is_l_int = self_compiler
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, l_tag, int_tag, "is_l_int")
                    .unwrap();
                let is_r_int = self_compiler
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, r_tag, int_tag, "is_r_int")
                    .unwrap();
                let both_int = self_compiler
                    .builder
                    .build_and(is_l_int, is_r_int, "both_int")
                    .unwrap();

                // check if both are strings
                let string_tag = self_compiler.context.i32_type().const_int(Tag::String as u64, false);
                let is_l_string = self_compiler
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, l_tag, string_tag, "is_l_string")
                    .unwrap();
                let is_r_string = self_compiler
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, r_tag, string_tag, "is_r_string")
                    .unwrap();

                // currently only handling int + int and string + string, for now didn't use a both_string variable
                let _both_string = self_compiler
                    .builder
                    .build_and(is_l_string, is_r_string, "both_string")
                    .unwrap();

                // create branches
                let parent_fn = self_compiler
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();
                let int_bb = self_compiler.context.append_basic_block(parent_fn, "add_int_bb");
                let string_bb = self_compiler.context.append_basic_block(parent_fn, "add_string_bb");
                let merge_bb = self_compiler.context.append_basic_block(parent_fn, "add_merge_bb");

                let _ = self_compiler
                    .builder
                    .build_conditional_branch(both_int, int_bb, string_bb);

                self_compiler.builder.position_at_end(int_bb);
                let l_int_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_int_data_ptr")
                    .unwrap();
                let l_int_val = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), l_int_data_ptr, "l_int_val")
                    .unwrap()
                    .into_int_value();

                let r_int_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, r_ptr, 1, "r_int_data_ptr")
                    .unwrap();
                let r_int_val = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), r_int_data_ptr, "r_int_val")
                    .unwrap()
                    .into_int_value();

                let int_sum = self_compiler
                    .builder
                    .build_int_add(l_int_val, r_int_val, "int_sum")
                    .unwrap();

                let int_res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "int_res_alloc")
                    .unwrap();
                let int_res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, int_res_ptr, 0, "int_res_tag_ptr")
                    .unwrap();
                self_compiler.builder.build_store(int_res_tag_ptr, int_tag).unwrap();
                let int_res_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, int_res_ptr, 1, "int_res_data_ptr")
                    .unwrap();
                self_compiler.builder.build_store(int_res_data_ptr, int_sum).unwrap();
                let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

                self_compiler.builder.position_at_end(string_bb);

                let l_str_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, l_ptr, 1, "l_str_data_ptr")
                    .unwrap();
                let l_str_ptr_int = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), l_str_data_ptr, "l_str_ptr_int")
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
                    .build_load(self_compiler.context.i64_type(), r_str_data_ptr, "r_str_ptr_int")
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

                self_compiler.builder
                    .build_memcpy(malloc_ptr, 1, l_str_ptr, 1, l_len_val)
                    .unwrap();

                let dest_ptr = unsafe {
                    self_compiler.builder
                        .build_gep(self_compiler.context.i8_type(), malloc_ptr, &[l_len_val], "dest_ptr")
                        .unwrap()
                };
                self_compiler.builder
                    .build_memcpy(dest_ptr, 1, r_str_ptr, 1, r_len_val)
                    .unwrap();

                let end_ptr = unsafe {
                    self_compiler.builder
                        .build_gep(self_compiler.context.i8_type(), malloc_ptr, &[total_len], "end_ptr")
                        .unwrap()
                };
                self_compiler.builder
                    .build_store(end_ptr, self_compiler.context.i8_type().const_int(0, false))
                    .unwrap();

                let str_res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "str_res_alloc")
                    .unwrap();

                let str_res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, str_res_ptr, 0, "str_res_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(str_res_tag_ptr, string_tag)
                    .unwrap();

                let str_res_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, str_res_ptr, 1, "str_res_data_ptr")
                    .unwrap();
                let malloc_ptr_as_i64 = self_compiler
                    .builder
                    .build_ptr_to_int(malloc_ptr, self_compiler.context.i64_type(), "malloc_ptr_as_i64")
                    .unwrap();
                self_compiler.builder
                    .build_store(str_res_data_ptr, malloc_ptr_as_i64)
                    .unwrap();

                let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
                self_compiler.builder.position_at_end(merge_bb);
                let phi = self_compiler
                    .builder
                    .build_phi(
                        self_compiler.context.ptr_type(AddressSpace::default()),
                        "add_res_phi",
                    )
                    .unwrap();
                phi.add_incoming(&[(&int_res_ptr, int_bb), (&str_res_ptr, string_bb)]);

                Ok(phi.as_basic_value())
}

pub fn create_mul_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(self_compiler, lhs, rhs, module, IntBinOp::Mul, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_mul(l_val, r_val, name).unwrap())
                })
}

pub fn create_minus_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(self_compiler, lhs, rhs, module, IntBinOp::Sub, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_sub(l_val, r_val, name).unwrap())
                })
}

pub fn create_div_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(self_compiler, lhs, rhs, module, IntBinOp::Div, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_signed_div(l_val, r_val, name).unwrap())
                })
}

pub fn create_mod_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    create_binary_int_op(self_compiler, lhs, rhs, module, IntBinOp::Mod, |builder, l_val, r_val, name| {
                    Ok(builder.build_int_signed_rem(l_val, r_val, name).unwrap())
                })
}

enum IntBinOp {
    Sub,
    Mul,
    Div,
    Mod,
}

fn create_binary_int_op<'ctx, F>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>, op: IntBinOp, op_fn: F) -> Result<BasicValueEnum<'ctx>, String>
where 
    F: Fn(&inkwell::builder::Builder<'ctx>, inkwell::values::IntValue<'ctx>, inkwell::values::IntValue<'ctx>, &str) -> Result<inkwell::values::IntValue<'ctx>, String>,
    {
        let l_ptr = self_compiler.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self_compiler.compile_expr(rhs, module)?.into_pointer_value();

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

                let result = op_fn(&self_compiler.builder, l_val, r_val, match op {
                    IntBinOp::Sub => "difference",
                    IntBinOp::Mul => "product",
                    IntBinOp::Div => "quotient",
                    IntBinOp::Mod => "remainder",
                })?;

                let res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        res_tag_ptr,
                        self_compiler.context
                            .i32_type()
                            .const_int(Tag::Integer as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                self_compiler.builder.build_store(res_data_ptr, result).unwrap();
                Ok(res_ptr.into())
    }

pub enum UpDown {
    Up = 0,
    Down = 1,
}

pub fn create_increment_or_decrement<'ctx>(self_compiler: &mut Compiler<'ctx>, expr: &ast::Expr, mode: UpDown, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let val_ptr = self_compiler.compile_expr(expr, module)?.into_pointer_value();

    let mode_str = match mode {
                    UpDown::Up => "increment",
                    UpDown::Down => "decrement",
                };

                let data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 1, format!("{}_data_ptr", mode_str).as_str())
                    .unwrap();
                let val = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), data_ptr, format!("{}_val", mode_str).as_str())
                    .unwrap()
                    .into_int_value();

                let one = self_compiler.context.i64_type().const_int(1, false);
                match mode {
                    UpDown::Up => {
                        let incremented = self_compiler
                            .builder
                            .build_int_add(val, one, "incremented")
                            .unwrap();
                        self_compiler.builder.build_store(data_ptr, incremented).unwrap();
                    }
                    UpDown::Down => {
                        let decremented = self_compiler
                            .builder
                            .build_int_sub(val, one, "decremented")
                            .unwrap();
                        self_compiler.builder.build_store(data_ptr, decremented).unwrap();
                    }
                }

                Ok(val_ptr.into())
}

pub enum EqNeq {
    Eq = 0,
    Neq = 1,
}

pub fn create_eq_or_neq<'ctx, F>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>,mode: EqNeq, op_fn: F) -> Result<BasicValueEnum<'ctx>, String> 
where 
    F: Fn(&inkwell::builder::Builder<'ctx>, inkwell::values::IntValue<'ctx>, inkwell::values::IntValue<'ctx>, &str) -> Result<inkwell::values::IntValue<'ctx>, String>,
{
    let l_ptr = self_compiler.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self_compiler.compile_expr(rhs, module)?.into_pointer_value();

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

                let result = op_fn(&self_compiler.builder, l_val, r_val, match mode {
                    EqNeq::Eq => "eq",
                    EqNeq::Neq => "neq",
                })?;

                let res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        res_tag_ptr,
                        self_compiler.context
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
                    .build_int_z_extend( result, self_compiler.context.i64_type(), "bool_as_i64")
                    .unwrap();
                self_compiler.builder.build_store(res_data_ptr, bool_as_i64).unwrap();

                Ok(res_ptr.into())
}

pub enum Comparison {
    Gt = 0,
    Lt = 1,
    Ge = 2,
    Le = 3,
}

pub fn create_comparison<'ctx, F>(self_compiler: &mut Compiler<'ctx>, lhs: &ast::Expr, rhs: &ast::Expr, module: &inkwell::module::Module<'ctx>, mode: Comparison, comp_fn: F) -> Result<BasicValueEnum<'ctx>, String> 
where 
    F: Fn(&inkwell::builder::Builder<'ctx>, inkwell::values::IntValue<'ctx>, inkwell::values::IntValue<'ctx>, &str) -> Result<inkwell::values::IntValue<'ctx>, String>,
{

let l_ptr = self_compiler.compile_expr(lhs, module)?.into_pointer_value();
                let r_ptr = self_compiler.compile_expr(rhs, module)?.into_pointer_value();

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

                let result = comp_fn(&self_compiler.builder, l_val, r_val, match mode {
                    Comparison::Gt => "gt",
                    Comparison::Lt => "lt",
                    Comparison::Ge => "ge",
                    Comparison::Le => "le",
                })?;

                let res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "res_alloc")
                    .unwrap();

                let res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        res_tag_ptr,
                        self_compiler.context
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
                self_compiler.builder.build_store(res_data_ptr, bool_as_i64).unwrap();
                Ok(res_ptr.into())

}

pub fn create_if_expr<'ctx>(self_compiler: &mut Compiler<'ctx>, cond: &ast::Expr, then_expr: &ast::Expr, else_expr: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let parent_fn = self_compiler
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();

                let then_bb = self_compiler.context.append_basic_block(parent_fn, "then_bb");
                let else_bb = self_compiler.context.append_basic_block(parent_fn, "else_bb");
                let merge_bb = self_compiler.context.append_basic_block(parent_fn, "if_merge");

                let cond_val = self_compiler.compile_expr(cond, module)?;
                let cond_ptr = cond_val.into_pointer_value();
                let cond_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, cond_ptr, 1, "cond_data_ptr")
                    .unwrap();
                let cond_loaded = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), cond_data_ptr, "cond_loaded")
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

pub fn create_list<'ctx>(self_compiler: &mut Compiler<'ctx>, elements: &Vec<ast::Expr>, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let list_ptr = self_compiler.build_list_from_exprs(elements, module)?;
                let i64_type = self_compiler.context.i64_type();

                let res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "list_res_alloc")
                    .unwrap();
                let res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        res_tag_ptr,
                        self_compiler.context.i32_type().const_int(Tag::List as u64, false),
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
                self_compiler.builder
                    .build_store(res_data_ptr, list_ptr_as_int)
                    .unwrap();

                Ok(res_ptr.into())
}

pub fn create_index<'ctx>(self_compiler: &mut Compiler<'ctx>, collection_expr: &ast::Expr, index_expr: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {

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
                    .build_load(self_compiler.context.i64_type(), list_data_ptr, "list_ptr_int")
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

                let index_val_ptr = self_compiler.compile_expr(index_expr, module)?.into_pointer_value();

                let index_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, index_val_ptr, 1, "index_data_ptr")
                    .unwrap();
                let index_int = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), index_data_ptr, "index_int")
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
                    ValueKind::Instruction(_) => {
                        Err("Expected basic value from __list_get".to_string())
                    }
                }
}

pub fn create_range<'ctx>(self_compiler: &mut Compiler<'ctx>, start_expr: &ast::Expr, end_expr: &ast::Expr, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let range_fn = self_compiler.get_runtime_fn(module, "__range_new");
                let start_val_ptr = self_compiler.compile_expr(start_expr, module)?.into_pointer_value();
                let start_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, start_val_ptr, 1, "start_data_ptr")
                    .unwrap();
                let start_int = self_compiler
                    .builder
                    .build_load(self_compiler.context.i64_type(), start_data_ptr, "start_int")
                    .unwrap()
                    .into_int_value();

                let end_val_ptr = self_compiler.compile_expr(end_expr, module)?.into_pointer_value();
                let end_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, end_val_ptr, 1, "end_data_ptr")
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

                let res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "range_res_alloc")
                    .unwrap();

                let res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        res_tag_ptr,
                        self_compiler.context.i32_type().const_int(Tag::Range as u64, false),
                    )
                    .unwrap();

                let res_data_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 1, "res_data_ptr")
                    .unwrap();
                let range_ptr_as_int = self_compiler
                    .builder
                    .build_ptr_to_int(range_ptr, self_compiler.context.i64_type(), "range_ptr_as_int")
                    .unwrap();
                self_compiler.builder
                    .build_store(res_data_ptr, range_ptr_as_int)
                    .unwrap();
                Ok(res_ptr.into())
}

pub fn create_module_access<'ctx>(self_compiler: &mut Compiler<'ctx>, module_name: &str, function_name: &str, args: &Vec<ast::Expr>, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
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

                let func_in_current_module = if let Some(func) = module.get_function(&function_name)
                {
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

                let result_val = match call_site.try_as_basic_value() {
                    ValueKind::Basic(val) => val,
                    ValueKind::Instruction(_) => {
                        return Err("Expected basic value from module function call".to_string());
                    }
                };

                let return_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "return_ptr")
                    .unwrap();
                self_compiler.builder.build_store(return_ptr, result_val).unwrap();
                Ok(return_ptr.into())
}

pub fn create_unit<'ctx>(self_compiler: &mut Compiler<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let res_ptr = self_compiler
                    .builder
                    .build_alloca(self_compiler.runtime_value_type, "unit_res")
                    .unwrap();
                let res_tag_ptr = self_compiler
                    .builder
                    .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                    .unwrap();
                self_compiler.builder
                    .build_store(
                        res_tag_ptr,
                        self_compiler.context.i32_type().const_int(Tag::Unit as u64, false),
                    )
                    .unwrap();
                Ok(res_ptr.into())
}

// !Define builtin macro handlers

pub fn call_builtin_macro_println<'ctx>(self_compiler: &mut Compiler<'ctx>, args: &Vec<ast::Expr>, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    let print_fn = self_compiler.get_runtime_fn(module, "__println");

                    let list_ptr = self_compiler.build_list_from_exprs(args, module)?;

                    self_compiler.builder
                        .build_call(print_fn, &[list_ptr.into()], "println_call")
                        .unwrap();

                    let res_ptr = self_compiler
                        .builder
                        .build_alloca(self_compiler.runtime_value_type, "unit_res")
                        .unwrap();
                    let res_tag_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                        .unwrap();
                    self_compiler.builder
                        .build_store(
                            res_tag_ptr,
                            self_compiler.context.i32_type().const_int(Tag::Unit as u64, false),
                        )
                        .unwrap();

                    return Ok(res_ptr.into());
}

pub fn call_builtin_macro_list_push<'ctx>(self_compiler: &mut Compiler<'ctx>, args: &Vec<ast::Expr>, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    if args.len() != 2 {
                        return Err("list_push expects 2 arguments".to_string());
                    }
                    let list_ptr = self_compiler.compile_expr(&args[0], module)?.into_pointer_value();
                    let val_ptr = self_compiler.compile_expr(&args[1], module)?.into_pointer_value();

                    let list_data_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, list_ptr, 1, "list_data_ptr")
                        .unwrap();
                    let list_vec_int = self_compiler
                        .builder
                        .build_load(self_compiler.context.i64_type(), list_data_ptr, "list_vec_int")
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
                    self_compiler.builder
                        .build_call(
                            list_push_fn,
                            &[list_vec_ptr.into(), val_tag.into(), val_data.into()],
                            "list_push_call",
                        )
                        .unwrap();

                    let res_ptr = self_compiler
                        .builder
                        .build_alloca(self_compiler.runtime_value_type, "unit_res")
                        .unwrap();
                    let res_tag_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, res_ptr, 0, "res_tag_ptr")
                        .unwrap();
                    self_compiler.builder
                        .build_store(
                            res_tag_ptr,
                            self_compiler.context.i32_type().const_int(Tag::Unit as u64, false),
                        )
                        .unwrap();

                    return Ok(res_ptr.into());
}

pub fn call_builtin_macro_clone<'ctx>(self_compiler: &mut Compiler<'ctx>, args: &Vec<ast::Expr>, module: &inkwell::module::Module<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
    if args.len() != 1 {
                        return Err("clone! expects 1 argument".to_string());
                    }
                    let arg_ptr = self_compiler.compile_expr(&args[0], module)?.into_pointer_value();

                    let tag_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, arg_ptr, 0, "clone_arg_tag_ptr")
                        .unwrap();
                    let tag = self_compiler
                        .builder
                        .build_load(self_compiler.context.i32_type(), tag_ptr, "clone_arg_tag")
                        .unwrap()
                        .into_int_value();

                    let data_ptr = self_compiler
                        .builder
                        .build_struct_gep(self_compiler.runtime_value_type, arg_ptr, 1, "clone_arg_data_ptr")
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
                        ValueKind::Instruction(_) => {
                            Err("Expected basic value from clone function".to_string())
                        }
                    };

                    let result_ptr = self_compiler
                        .builder
                        .build_alloca(self_compiler.runtime_value_type, "clone_res_alloc")
                        .unwrap();

                    self_compiler.builder.build_store(result_ptr, result_val?).unwrap();

                    return Ok(result_ptr.into());
}
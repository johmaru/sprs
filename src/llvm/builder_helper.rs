// BuilderExt / ContextExt traits — shared across all builder submodules.

use inkwell::{
    types::StructType,
    values::{BasicValueEnum, FunctionValue, InstructionValue, IntValue, PointerValue},
};

use crate::llvm::compiler::{Compiler, Tag};

/// Builder extensions for GEP, tag load/store, and bitwise or.
pub(crate) trait BuilderExt<'ctx> {
    fn build_tag_gep(&self, ptr: PointerValue<'ctx>, name: &str) -> PointerValue<'ctx>;
    fn build_data_gep(&self, ptr: PointerValue<'ctx>, name: &str) -> PointerValue<'ctx>;
    fn get_current_function(&self) -> FunctionValue<'ctx>;
    fn build_load_tag(&self, ptr: PointerValue<'ctx>, name: &str) -> IntValue<'ctx>;
    fn build_tag_store(&self, tag: Tag, ptr: PointerValue<'ctx>) -> InstructionValue<'ctx>;
    fn build_load(
        &self,
        pointee_ty: StructType<'ctx>,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> BasicValueEnum<'ctx>;
    fn or(&self, lhs: IntValue<'ctx>, rhs: IntValue<'ctx>, name: &str) -> IntValue<'ctx>;
}

impl<'ctx> BuilderExt<'ctx> for Compiler<'ctx> {
    fn build_tag_gep(&self, ptr: PointerValue<'ctx>, name: &str) -> PointerValue<'ctx> {
        self.builder
            .build_struct_gep(
                self.runtime_value_type,
                ptr,
                0,
                &format!("{}_tag_ptr", name),
            )
            .unwrap()
    }

    fn build_data_gep(&self, ptr: PointerValue<'ctx>, name: &str) -> PointerValue<'ctx> {
        self.builder
            .build_struct_gep(
                self.runtime_value_type,
                ptr,
                1,
                &format!("{}_data_ptr", name),
            )
            .unwrap()
    }

    fn or(&self, lhs: IntValue<'ctx>, rhs: IntValue<'ctx>, name: &str) -> IntValue<'ctx> {
        self.builder
            .build_or(lhs, rhs, &format!("{}_or", name))
            .unwrap()
    }

    fn get_current_function(&self) -> FunctionValue<'ctx> {
        self.builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap()
    }

    fn build_load_tag(&self, tag_ptr: PointerValue<'ctx>, name: &str) -> IntValue<'ctx> {
        self.builder
            .build_load(
                self.context.i32_type(),
                tag_ptr,
                &format!("{}_loaded_tag", name),
            )
            .unwrap()
            .into_int_value()
    }

    fn build_tag_store(&self, tag: Tag, ptr: PointerValue<'ctx>) -> InstructionValue<'ctx> {
        self.builder
            .build_store(ptr, self.context.i32_type().const_int(tag as u64, false))
            .unwrap()
    }

    fn build_load(
        &self,
        pointee_ty: StructType<'ctx>,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        self.builder.build_load(pointee_ty, ptr, name).unwrap()
    }
}

/// Context extensions for tag enum constants and comparisons.
pub(crate) trait ContextExt<'ctx> {
    fn get_tag_from_tag_enum(&self, tag: Tag) -> IntValue<'ctx>;
    fn tag_cmp(
        &self,
        op: inkwell::IntPredicate,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
        name: &str,
    ) -> IntValue<'ctx>;
}

impl<'ctx> ContextExt<'ctx> for Compiler<'ctx> {
    fn get_tag_from_tag_enum(&self, tag: Tag) -> IntValue<'ctx> {
        self.context.i32_type().const_int(tag as u64, false)
    }

    fn tag_cmp(
        &self,
        op: inkwell::IntPredicate,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
        name: &str,
    ) -> IntValue<'ctx> {
        self.builder
            .build_int_compare(op, lhs, rhs, &format!("{}_tag_cmp", name))
            .unwrap()
    }
}

// Re-export all submodules so `builder_helper::create_xxx` paths still work.
pub use crate::llvm::arithmetic::*;
pub use crate::llvm::comparison::*;
pub use crate::llvm::control_flow::*;
pub use crate::llvm::data_structures::*;
pub use crate::llvm::macros::*;
pub use crate::llvm::value::*;
pub use crate::llvm::variable::*;

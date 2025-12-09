// interpreter currently not support yet, for now this file set a allowed unused
#![allow(unused)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Any,
    Int,
    Float,
    Bool,
    Str,
    Unit,

    // System types
    TypeI8,
    TypeU8,
    TypeI16,
    TypeU16,
    TypeI32,
    TypeU32,
    TypeI64,
    TypeU64,

    TypeF16,
    TypeF32,
    TypeF64,
}

pub fn is_int_type_in_llvm() -> Vec<Type> {
    vec![
        Type::Int,
        Type::TypeI8,
        Type::TypeU8,
        Type::TypeI16,
        Type::TypeU16,
        Type::TypeI32,
        Type::TypeU32,
        Type::TypeI64,
        Type::TypeU64,
        Type::Float,
        Type::TypeF16,
        Type::TypeF32,
        Type::TypeF64,
        Type::Bool,
    ]
}

pub fn not_int_type_in_llvm() -> Vec<Type> {
    vec![
        Type::TypeF16,
        Type::TypeF32,
        Type::TypeF64,
        Type::Str,
        Type::Bool,
        Type::Unit,
    ]
}

pub fn is_float_type_in_llvm() -> Vec<Type> {
    vec![Type::Float, Type::TypeF16, Type::TypeF32, Type::TypeF64]
}

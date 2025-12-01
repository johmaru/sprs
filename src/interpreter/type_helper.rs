// interpreter currently not support yet, for now this file set a allowed unused
#![allow(unused)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Any,
    Int,
    Bool,
    Str,
    Unit,
}

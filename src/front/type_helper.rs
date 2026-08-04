/// Compile-time type used by the hybrid type layer.
///
/// Runtime values are still `{ tag, data }` (`Tag` in `llvm/compiler.rs`).
/// `Type` is the static knowledge attached to bindings and expressions.
/// When a type is monomorphic at runtime, [`Type::tag_discriminant`] matches
/// the corresponding `Tag as u32` value.
///
/// Correspondence (`Type` → `Tag`):
/// | Type            | Tag          | discriminant |
/// |-----------------|--------------|--------------|
/// | Int             | Integer      | 0            |
/// | Float           | Float        | 1            |
/// | Str             | String       | 2            |
/// | Bool            | Boolean      | 3            |
/// | List            | List         | 4            |
/// | Range           | Range        | 5            |
/// | Unit            | Unit         | 6            |
/// | Enum            | Enum         | 7            |
/// | Struct(_)       | Struct       | 8            |
/// | Error           | Error        | 9            |
/// | TypeI8          | Int8         | 100          |
/// | TypeU8          | Uint8        | 101          |
/// | TypeI16         | Int16        | 102          |
/// | TypeU16         | Uint16       | 103          |
/// | TypeI32         | Int32        | 104          |
/// | TypeU32         | Uint32       | 105          |
/// | TypeI64         | Int64        | 106          |
/// | TypeU64         | Uint64       | 107          |
/// | TypeF16         | Float16      | 108          |
/// | TypeF32         | Float32      | 109          |
/// | TypeF64         | Float64      | 110          |
/// | Any             | (none)       | (none)       |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Any,
    Int,
    Float,
    Bool,
    Str,
    List,
    Range,
    Unit,
    Enum,
    Struct(String),
    Error,

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

impl Type {
    /// Runtime `Tag` discriminant for this type, if any.
    ///
    /// `Any` has no tag. `Struct(name)` maps to Struct (`8`) regardless of name.
    /// Values must stay in sync with `Tag` in `llvm/compiler.rs`.
    pub fn tag_discriminant(&self) -> Option<u32> {
        match self {
            Type::Any => None,
            Type::Int => Some(0),
            Type::Float => Some(1),
            Type::Str => Some(2),
            Type::Bool => Some(3),
            Type::List => Some(4),
            Type::Range => Some(5),
            Type::Unit => Some(6),
            Type::Enum => Some(7),
            Type::Struct(_) => Some(8),
            Type::Error => Some(9),
            Type::TypeI8 => Some(100),
            Type::TypeU8 => Some(101),
            Type::TypeI16 => Some(102),
            Type::TypeU16 => Some(103),
            Type::TypeI32 => Some(104),
            Type::TypeU32 => Some(105),
            Type::TypeI64 => Some(106),
            Type::TypeU64 => Some(107),
            Type::TypeF16 => Some(108),
            Type::TypeF32 => Some(109),
            Type::TypeF64 => Some(110),
        }
    }

    /// Static type for a runtime `Tag` discriminant.
    ///
    /// Struct (`8`) becomes `Type::Struct(String::new())` because the struct
    /// name is not stored in the tag.
    pub fn from_tag_discriminant(disc: u32) -> Option<Type> {
        match disc {
            0 => Some(Type::Int),
            1 => Some(Type::Float),
            2 => Some(Type::Str),
            3 => Some(Type::Bool),
            4 => Some(Type::List),
            5 => Some(Type::Range),
            6 => Some(Type::Unit),
            7 => Some(Type::Enum),
            8 => Some(Type::Struct(String::new())),
            9 => Some(Type::Error),
            100 => Some(Type::TypeI8),
            101 => Some(Type::TypeU8),
            102 => Some(Type::TypeI16),
            103 => Some(Type::TypeU16),
            104 => Some(Type::TypeI32),
            105 => Some(Type::TypeU32),
            106 => Some(Type::TypeI64),
            107 => Some(Type::TypeU64),
            108 => Some(Type::TypeF16),
            109 => Some(Type::TypeF32),
            110 => Some(Type::TypeF64),
            _ => None,
        }
    }
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
    ]
}

pub fn not_int_type_in_llvm() -> Vec<Type> {
    vec![
        Type::Float,
        Type::TypeF16,
        Type::TypeF32,
        Type::TypeF64,
        Type::Str,
        Type::List,
        Type::Range,
        Type::Unit,
        Type::Bool,
        Type::Error,
    ]
}

pub fn is_float_type_in_llvm() -> Vec<Type> {
    vec![Type::Float, Type::TypeF16, Type::TypeF32, Type::TypeF64]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_discriminants_match_known_tag_values() {
        assert_eq!(Type::Int.tag_discriminant(), Some(0));
        assert_eq!(Type::List.tag_discriminant(), Some(4));
        assert_eq!(Type::Range.tag_discriminant(), Some(5));
        assert_eq!(Type::Error.tag_discriminant(), Some(9));
        assert_eq!(Type::Any.tag_discriminant(), None);
        assert_eq!(
            Type::from_tag_discriminant(4),
            Some(Type::List)
        );
        assert_eq!(
            Type::from_tag_discriminant(9),
            Some(Type::Error)
        );
    }
}

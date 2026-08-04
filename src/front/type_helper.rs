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
/// | App(name, args) | (none)       | (none)       |
/// | Param(name)     | (none)       | (none)       |
///
/// `App` / `Param` are compile-time only: inputs to checking and (later)
/// monomorphization (#29). They are not LLVM types and not runtime tags.
///
/// Flat `List` / `Range` / `Error` coexist with parametric forms such as
/// `App("List", [Int])`. Everyday annotations keep the keywords (`list`,
/// `err`); `App` is for explicit constructor application in annotations.
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
    Enum(String),
    Struct(String),
    Error,

    App(String, Vec<Type>),
    Param(String),

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

/// A type annotation as written in source (`>> int`, `>> ambi int`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAnnot {
    pub ty: Type,
    /// When true, the binding starts as `ty` but reassignment may widen it dynamically.
    pub ambi: bool,
}

impl Type {
    /// Runtime `Tag` discriminant for this type, if any.
    ///
    /// `Any` has no tag. `Struct(name)` maps to Struct (`8`) regardless of name.
    /// `App` / `Param` never have a tag.
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
            Type::Enum(_) => Some(7),
            Type::Struct(_) => Some(8),
            Type::Error => Some(9),
            Type::App(_, _) => None,
            Type::Param(_) => None,
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
    /// There is no discriminant for `App` / `Param`.
    pub fn from_tag_discriminant(disc: u32) -> Option<Type> {
        match disc {
            0 => Some(Type::Int),
            1 => Some(Type::Float),
            2 => Some(Type::Str),
            3 => Some(Type::Bool),
            4 => Some(Type::List),
            5 => Some(Type::Range),
            6 => Some(Type::Unit),
            7 => Some(Type::Enum(String::new())),
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

/// Whether two static types are interchangeable for checking.
///
/// Rules:
/// - `Any` is compatible with every type (either side)
/// - `Int` ≡ `TypeI64` (language default integer is i64)
/// - `Float` ≡ `TypeF64` (language default float is f64)
/// - `Struct` names must match; empty name (from tag recovery) matches any struct
/// - `App(n1, a1)` ≡ `App(n2, a2)` when names and arities match and each
///   argument pair is compatible (recursively)
/// - `Param(n1)` ≡ `Param(n2)` only when names match (no substitution yet; #29)
/// - Flat monomorphic forms bridge empty / `Any`-arg applications:
///   - `List` ≡ `App("List", [])` ≡ `App("List", [Any])`
///   - `Range` ≡ `App("Range", [])` ≡ `App("Range", [Any])`
///   - `Error` ≡ `App("Error", [])`
///   `App("List", [Int])` is not compatible with bare `List`
/// - otherwise exact equality
pub fn types_compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual {
        return true;
    }
    if matches!(expected, Type::Any) || matches!(actual, Type::Any) {
        return true;
    }
    if is_default_int(expected) && is_default_int(actual) {
        return true;
    }
    if is_default_float(expected) && is_default_float(actual) {
        return true;
    }
    match (expected, actual) {
        (Type::Struct(a), Type::Struct(b)) => a.is_empty() || b.is_empty() || a == b,
        (Type::App(n1, a1), Type::App(n2, a2)) => {
            n1 == n2
                && a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|(x, y)| types_compatible(x, y))
        }
        (Type::Param(a), Type::Param(b)) => a == b,
        (Type::List, Type::App(n, args)) | (Type::App(n, args), Type::List) => {
            n == "List" && is_untyped_collection_args(args)
        }
        (Type::Range, Type::App(n, args)) | (Type::App(n, args), Type::Range) => {
            n == "Range" && is_untyped_collection_args(args)
        }
        (Type::Error, Type::App(n, args)) | (Type::App(n, args), Type::Error) => {
            n == "Error" && args.is_empty()
        }
        _ => false,
    }
}

/// Args that still mean “monomorphic / untyped” collection in the flat sense.
fn is_untyped_collection_args(args: &[Type]) -> bool {
    match args {
        [] => true,
        [Type::Any] => true,
        _ => false,
    }
}

fn is_default_int(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::TypeI64)
}

fn is_default_float(ty: &Type) -> bool {
    matches!(ty, Type::Float | Type::TypeF64)
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
        Type::App(String::new(), Vec::new()),
        Type::Param(String::new()),
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
        assert_eq!(Type::App("List".into(), vec![Type::Int]).tag_discriminant(), None);
        assert_eq!(Type::Param("T".into()).tag_discriminant(), None);
        assert_eq!(Type::from_tag_discriminant(4), Some(Type::List));
        assert_eq!(Type::from_tag_discriminant(9), Some(Type::Error));
        assert_eq!(Type::from_tag_discriminant(10), None);
        assert_eq!(Type::from_tag_discriminant(11), None);
    }

    #[test]
    fn types_compatible_int_equals_i64() {
        assert!(types_compatible(&Type::Int, &Type::TypeI64));
        assert!(types_compatible(&Type::TypeI64, &Type::Int));
        assert!(types_compatible(&Type::Float, &Type::TypeF64));
        assert!(!types_compatible(&Type::Int, &Type::TypeI32));
        assert!(!types_compatible(&Type::Int, &Type::Str));
        assert!(types_compatible(&Type::Any, &Type::Str));
        assert!(types_compatible(
            &Type::Struct("A".into()),
            &Type::Struct("A".into())
        ));
        assert!(!types_compatible(
            &Type::Struct("A".into()),
            &Type::Struct("B".into())
        ));
    }

    #[test]
    fn types_compatible_app_by_name_and_args() {
        let list_int = Type::App("List".into(), vec![Type::Int]);
        let list_i64 = Type::App("List".into(), vec![Type::TypeI64]);
        let list_str = Type::App("List".into(), vec![Type::Str]);
        let result_int_err = Type::App("Result".into(), vec![Type::Int, Type::Error]);
        let result_i64_err = Type::App("Result".into(), vec![Type::TypeI64, Type::Error]);

        assert!(types_compatible(&list_int, &list_i64));
        assert!(!types_compatible(&list_int, &list_str));
        assert!(types_compatible(&result_int_err, &result_i64_err));
        assert!(!types_compatible(
            &result_int_err,
            &Type::App("Result".into(), vec![Type::Int])
        ));
        assert!(!types_compatible(&list_int, &result_int_err));
    }

    #[test]
    fn types_compatible_list_bridges_empty_app() {
        assert!(types_compatible(
            &Type::List,
            &Type::App("List".into(), vec![])
        ));
        assert!(types_compatible(
            &Type::App("List".into(), vec![Type::Any]),
            &Type::List
        ));
        assert!(!types_compatible(
            &Type::List,
            &Type::App("List".into(), vec![Type::Int])
        ));
        assert!(types_compatible(
            &Type::Error,
            &Type::App("Error".into(), vec![])
        ));
        assert!(!types_compatible(
            &Type::Error,
            &Type::App("Error".into(), vec![Type::Int])
        ));
    }

    #[test]
    fn types_compatible_param_same_name_only() {
        assert!(types_compatible(
            &Type::Param("T".into()),
            &Type::Param("T".into())
        ));
        assert!(!types_compatible(
            &Type::Param("T".into()),
            &Type::Param("U".into())
        ));
        assert!(!types_compatible(&Type::Param("T".into()), &Type::Int));
    }
}

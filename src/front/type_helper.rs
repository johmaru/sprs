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
/// | Label           | Label        | 10           |
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
/// | Atom(name)      | (none)       | (none)       |
///
/// `App` / `Param` / `Atom` are compile-time only: inputs to checking and
/// (later) monomorphization (#29). They are not LLVM types and not runtime
/// tags. `Atom` carries a label name as written in a type argument
/// (`Label(:ok)`); it has no tag of its own.
///
/// Flat `List` / `Range` / `Label` coexist with parametric forms such as
/// `App("List", [Int])`. Everyday annotations keep the keywords (`list`,
/// `err`, `label`); `App` is for explicit constructor application in annotations.
///
/// Runtime tag 9 is unused: it was the legacy `Error` tag, removed in Phase 3
/// Step 3. No `Type` maps to it.
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
    Label,

    App(String, Vec<Type>),
    Param(String),
    Atom(String), // compile-time only: `:name` in type args. No tag_discriminant.

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
            Type::Label => Some(10),
            Type::App(_, _) => None,
            Type::Param(_) => None,
            Type::Atom(_) => None,
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
            // 9 is the legacy Error tag (removed in Phase 3 Step 3); no Type maps to it.
            10 => Some(Type::Label),
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
/// - `Atom(a)` ≡ `Atom(b)` only when `a == b` (label names are exact)
/// - Named label applications:
///   - `Label(:name)` ≡ `Label(:name, any)` (symmetric)
///   - `Label(:name, T)` accepts `Label(:name)` — the expected side narrows
///     the payload; the reverse (`Label(:name)` vs `Label(:name, T)`) is not
///   - `App("Label", [Int])` (name free, payload `Int`) never matches
///     `Label(:name, Int)` — a bare `Int` arg is not an `Atom`
/// - Flat monomorphic forms bridge empty / `Any`-arg applications:
///   - `List` ≡ `App("List", [])` ≡ `App("List", [Any])`
///   - `Range` ≡ `App("Range", [])` ≡ `App("Range", [Any])`
///   - `Label` ≡ `App("Label", [])` ≡ `App("Label", [Any])`
///   `App("List", [Int])` is not compatible with bare `List`; bare `Label` is
///   not compatible with `Label(:name[, T])` either
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
        (Type::Atom(a), Type::Atom(b)) => a == b,
        (Type::App(n1, a1), Type::App(n2, a2)) if n1 == "Label" && n2 == "Label" => {
            label_args_compatible(a1, a2)
        }
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
        (Type::Label, Type::App(n, args)) | (Type::App(n, args), Type::Label) => {
            n == "Label" && is_untyped_collection_args(args)
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

/// Compatibility for `Label(:name[, T])` argument lists.
///
/// - `Label(:name)` ≡ `Label(:name, any)` (symmetric)
/// - `Label(:name, T)` accepts `Label(:name)` — the expected side narrows the
///   payload; the reverse (`Label(:name)` vs `Label(:name, T)`) is not
/// - same-arity lists recurse; a bare `Int` never matches `Atom(name)`
fn label_args_compatible(expected: &[Type], actual: &[Type]) -> bool {
    match (expected, actual) {
        ([Type::Atom(e_name), Type::Any], [Type::Atom(a_name)]) => e_name == a_name,
        ([Type::Atom(e_name)], [Type::Atom(a_name), Type::Any]) => e_name == a_name,
        ([Type::Atom(e_name), _], [Type::Atom(a_name)]) => e_name == a_name,
        (e, a) if e.len() == a.len() => e
            .iter()
            .zip(a.iter())
            .all(|(x, y)| types_compatible(x, y)),
        _ => false,
    }
}

/// Whether a static type denotes the `:error` label.
///
/// Matches the `err` sugar (`Label(:error)`) and both named forms
/// `Label(:error)` / `Label(:error, T)`. Used for the failure check in
/// return-type validation (`@error(reason)` produces `{:error, reason}`).
pub fn is_error_label_type(ty: &Type) -> bool {
    match ty {
        Type::App(n, args) if n == "Label" => match args.as_slice() {
            [Type::Atom(name)] | [Type::Atom(name), _] => name == "error",
            _ => false,
        },
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
        Type::Label,
        Type::App(String::new(), Vec::new()),
        Type::Param(String::new()),
        Type::Atom(String::new()),
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
        assert_eq!(Type::Label.tag_discriminant(), Some(10));
        assert_eq!(Type::Any.tag_discriminant(), None);
        assert_eq!(Type::Atom("ok".into()).tag_discriminant(), None);
        assert_eq!(Type::App("List".into(), vec![Type::Int]).tag_discriminant(), None);
        assert_eq!(Type::Param("T".into()).tag_discriminant(), None);
        assert_eq!(Type::from_tag_discriminant(4), Some(Type::List));
        // 9 is the legacy Error tag (removed); no Type maps to it.
        assert_eq!(Type::from_tag_discriminant(9), None);
        assert_eq!(Type::from_tag_discriminant(10), Some(Type::Label));
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
        let err_label = Type::App("Label".into(), vec![Type::Atom("error".into())]);
        let result_int_err = Type::App("Result".into(), vec![Type::Int, err_label.clone()]);
        let result_i64_err = Type::App("Result".into(), vec![Type::TypeI64, err_label]);

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
            &Type::Label,
            &Type::App("Label".into(), vec![])
        ));
        assert!(types_compatible(
            &Type::App("Label".into(), vec![Type::Any]),
            &Type::Label
        ));
        assert!(!types_compatible(
            &Type::Label,
            &Type::App("Label".into(), vec![Type::Int])
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

    #[test]
    fn types_compatible_named_labels() {
        let label_ok = Type::App("Label".into(), vec![Type::Atom("ok".into())]);
        let label_ok_any = Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Any]);
        let label_ok_int = Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Int]);
        let label_err = Type::App("Label".into(), vec![Type::Atom("error".into())]);
        let label_free_int = Type::App("Label".into(), vec![Type::Int]);
        let label_int_str = Type::App("Label".into(), vec![Type::Int, Type::Str]);

        // Atom names match exactly
        assert!(types_compatible(
            &Type::Atom("ok".into()),
            &Type::Atom("ok".into())
        ));
        assert!(!types_compatible(
            &Type::Atom("ok".into()),
            &Type::Atom("err".into())
        ));
        assert!(!types_compatible(&Type::Atom("ok".into()), &Type::Int));

        // Label(:name) ≡ Label(:name, any), both directions
        assert!(types_compatible(&label_ok, &label_ok_any));
        assert!(types_compatible(&label_ok_any, &label_ok));

        // Label(:ok, int) accepts Label(:ok) (expected narrows payload)…
        assert!(types_compatible(&label_ok_int, &label_ok));
        // …but the reverse does not
        assert!(!types_compatible(&label_ok, &label_ok_int));

        // names must match
        assert!(!types_compatible(&label_ok, &label_err));
        assert!(!types_compatible(
            &Type::App(
                "Label".into(),
                vec![Type::Atom("ok".into()), Type::Int]
            ),
            &label_err
        ));

        // bare Int arg (name free) never matches Atom(name)
        assert!(!types_compatible(&label_free_int, &label_ok_int));
        assert!(!types_compatible(&label_ok_int, &label_free_int));
        assert!(!types_compatible(&label_int_str, &label_ok_int));

        // bare Label stays incompatible with named forms
        assert!(!types_compatible(&Type::Label, &label_ok));
        assert!(!types_compatible(&label_ok_int, &Type::Label));
    }

    #[test]
    fn is_error_label_type_matches_err_sugar_and_named_forms() {
        let err_sugar = Type::App("Label".into(), vec![Type::Atom("error".into())]);
        let err_payload = Type::App(
            "Label".into(),
            vec![Type::Atom("error".into()), Type::Str],
        );
        let ok_label = Type::App("Label".into(), vec![Type::Atom("ok".into())]);
        let ok_payload = Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Int]);

        assert!(is_error_label_type(&err_sugar));
        assert!(is_error_label_type(&err_payload));
        assert!(!is_error_label_type(&ok_label));
        assert!(!is_error_label_type(&ok_payload));
        assert!(!is_error_label_type(&Type::Label));
        assert!(!is_error_label_type(&Type::Any));
        assert!(!is_error_label_type(
            &Type::App("Result".into(), vec![Type::Int, err_sugar.clone()])
        ));
    }
}

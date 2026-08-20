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
/// | ClosedLabelSet(_) | Atom      | 9            |
/// | Struct(_)       | Struct       | 8            |
/// | 7              | (unused)     | (was Enum)   |
/// | Label           | Label        | 10           |
/// | AtomVal         | Atom         | 9            |
/// | Buffer          | Buffer       | 11           |
/// | RawPtr          | RawPtr       | 12           |
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
/// substitution. They are not LLVM types. `tag_discriminant` is `None` for
/// `App`; [`Type::runtime_shape`] recovers `Tag::List` / `Tag::Label` for
/// those constructors. `Process(T)` has no runtime tag yet (#25).
/// `Atom` carries a label name as written in a type argument (`Label(:ok)`).
///
/// Flat `List` / `Range` / `Label` coexist with parametric forms such as
/// `App("List", [Int])`. Surface annotations use `List(T)` / `List(Any)`;
/// `Type::List` remains an internal alias of `List(Any)`.
///
/// Runtime tag 9 is `Atom`: an interned, immutable symbol with no payload
/// (`Tag::Atom`, data = intern id). It is an immediate value, not a slab
/// handle — `is_heap_tag` excludes it and `__drop` / `__clone` are no-ops.
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
    ClosedLabelSet(String),
    Struct(String),
    Label,
    AtomVal,
    Buffer,
    RawPtr,

    App(String, Vec<Type>),
    Param(String),
    Atom(String), // compile-time only: `:name` in type args. No tag_discriminant.
    /// Unresolved bare struct name from a type annotation. Compile-time only.
    Named(String),
    /// Unresolved `Self` in a struct field annotation. Compile-time only.
    SelfType,

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
        self.base_tag_discriminant()
    }

    fn base_tag_discriminant(&self) -> Option<u32> {
        match self {
            Type::Any => None,
            Type::Int => Some(0),
            Type::Float => Some(1),
            Type::Str => Some(2),
            Type::Bool => Some(3),
            Type::List => Some(4),
            Type::Range => Some(5),
            Type::Unit => Some(6),
            Type::ClosedLabelSet(_) => Some(9),
            Type::Struct(_) => Some(8),
            Type::AtomVal => Some(9),
            Type::Label => None,
            Type::Buffer => Some(11),
            Type::RawPtr => Some(12),
            Type::App(_, _) => None,
            Type::Param(_) => None,
            Type::Atom(_) => None,
            Type::Named(_) => None,
            Type::SelfType => None,
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

    /// Runtime lowering shape. Distinct from `tag_discriminant`: constructor
    /// applications do not store a tag on `App` itself.
    #[allow(dead_code)]
    pub fn runtime_shape(&self) -> Option<u32> {
        match self {
            Type::App(name, _) if name == "List" => Some(4),
            Type::App(name, _) if name == "Label" => Some(10),
            Type::App(_, _) => None,
            other => other.base_tag_discriminant(),
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
            7 => None,
            8 => Some(Type::Struct(String::new())),
            9 => Some(Type::AtomVal),
            10 => Some(Type::Label),
            11 => Some(Type::Buffer),
            12 => Some(Type::RawPtr),
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

impl std::fmt::Display for Type {
    /// Canonical surface spelling of a type. `Int`/`Float` render as their
    /// default widths (`i64`/`f64`); `App` renders constructor application
    /// with `List(T)` / `Label(:name, T)` spelled canonically.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Any => write!(formatter, "Any"),
            Type::Int => write!(formatter, "i64"),
            Type::Float => write!(formatter, "f64"),
            Type::Bool => write!(formatter, "bool"),
            Type::Str => write!(formatter, "str"),
            Type::List => write!(formatter, "List"),
            Type::Range => write!(formatter, "Range"),
            Type::Unit => write!(formatter, "unit"),
            Type::ClosedLabelSet(name) | Type::Struct(name) | Type::Named(name) => {
                write!(formatter, "{name}")
            }
            Type::Label => write!(formatter, "Label"),
            Type::AtomVal => write!(formatter, "Atom"),
            Type::Buffer => write!(formatter, "Buffer"),
            Type::RawPtr => write!(formatter, "RawPtr"),
            Type::App(name, args) => {
                let args = args
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(formatter, "{name}({args})")
            }
            Type::Param(name) => write!(formatter, "{name}"),
            Type::Atom(name) => write!(formatter, ":{name}"),
            Type::SelfType => write!(formatter, "Self"),
            Type::TypeI8 => write!(formatter, "i8"),
            Type::TypeU8 => write!(formatter, "u8"),
            Type::TypeI16 => write!(formatter, "i16"),
            Type::TypeU16 => write!(formatter, "u16"),
            Type::TypeI32 => write!(formatter, "i32"),
            Type::TypeU32 => write!(formatter, "u32"),
            Type::TypeI64 => write!(formatter, "i64"),
            Type::TypeU64 => write!(formatter, "u64"),
            Type::TypeF16 => write!(formatter, "f16"),
            Type::TypeF32 => write!(formatter, "f32"),
            Type::TypeF64 => write!(formatter, "f64"),
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
/// - `ClosedLabelSet` names must match (`:Color.red` is an Atom at runtime)
/// - `App(n1, a1)` ≡ `App(n2, a2)` when names and arities match and each
///   argument pair is compatible (recursively)
/// - `Param(n1)` ≡ `Param(n2)` only when names match
/// - `Atom(a)` ≡ `Atom(b)` only when `a == b` (label names are exact)
/// - Broad `Label` accepts payload-less atoms (`:name`), payload labels
///   (`Label(:name, T)`), closed label sets, and the runtime-only flat
///   `AtomVal` — it is the surface union of runtime tags 9 and 10
/// - `Label(:name, T)` applications compare name and payload recursively;
///   arity differences are incompatible
/// - Flat monomorphic forms bridge empty / `Any`-arg applications:
///   - `List` ≡ `App("List", [])` ≡ `App("List", [Any])`
///   - `Range` ≡ `App("Range", [])` ≡ `App("Range", [Any])`
///   `App("List", [Int])` is not compatible with bare `List`
/// - `AtomVal` is not a constructor name on the surface; the old
///   `App("Atom", ...)` bridge has been removed
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
        (Type::ClosedLabelSet(a), Type::ClosedLabelSet(b)) => a == b,
        (Type::Atom(a), Type::Atom(b)) => a == b,
        // Broad `Label` is the surface union of payload-less atoms, payload
        // labels, closed label set members, and the runtime-only flat `Atom`.
        (Type::Label, Type::Atom(_)) | (Type::Atom(_), Type::Label) => true,
        (Type::Label, Type::ClosedLabelSet(_)) | (Type::ClosedLabelSet(_), Type::Label) => true,
        (Type::Label, Type::AtomVal) | (Type::AtomVal, Type::Label) => true,
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
            n == "Label"
                && (is_untyped_collection_args(args)
                    || matches!(args.first(), Some(Type::Atom(_))))
        }
        _ => false,
    }
}

/// Directional assignability: can `actual` be used where `expected` is required.
///
/// Same as [`types_compatible`] except `List`:
/// `List(T)` widens to `List(Any)`; `List(Any)` does not narrow to `List(T)`.
/// Top-level `Any` (unannotated / unknown) is still accepted either side.
pub fn types_assignable(expected: &Type, actual: &Type) -> bool {
    match (list_element(expected), list_element(actual)) {
        (Some(exp_elem), Some(act_elem)) => {
            if matches!(exp_elem, Type::Any) {
                return true;
            }
            if matches!(act_elem, Type::Any) {
                return false;
            }
            types_compatible(exp_elem, act_elem)
        }
        _ => types_compatible(expected, actual),
    }
}

/// Element type of `List(T)` / `List(Any)` / internal flat `List`.
const ANY_TYPE: Type = Type::Any;

pub fn list_element(ty: &Type) -> Option<&Type> {
    match ty {
        Type::List => Some(&ANY_TYPE),
        Type::App(name, args) if name == "List" => match args.as_slice() {
            [] => Some(&ANY_TYPE),
            [elem] => Some(elem),
            _ => None,
        },
        _ => None,
    }
}

/// Result type tracked by `Process(T)`, if this is a process constructor app.
#[allow(dead_code)]
pub fn process_result_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::App(name, args) if name == "Process" => match args.as_slice() {
            [result] => Some(result),
            _ => None,
        },
        _ => None,
    }
}

/// Join element types from a list literal (no expected type).
pub fn join_list_element_types(elems: &[Type]) -> Type {
    let mut joined: Option<Type> = None;
    for elem in elems {
        joined = Some(match joined {
            None => elem.clone(),
            Some(Type::Any) => Type::Any,
            Some(prev) if types_compatible(&prev, elem) => {
                if matches!(elem, Type::Any) {
                    Type::Any
                } else {
                    prev
                }
            }
            Some(_) => Type::Any,
        });
    }
    joined.unwrap_or(Type::Any)
}

pub fn list_type(element: Type) -> Type {
    Type::App("List".into(), vec![element])
}

#[allow(dead_code)]
pub fn process_type(result: Type) -> Type {
    Type::App("Process".into(), vec![result])
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
/// - exact `:name` forms match by name only
/// - `Label(:name, T)` recurses into the payload type
/// - arity differences are incompatible (a payload-less `Label(:name)` is no
///   longer a valid surface type)
fn label_args_compatible(expected: &[Type], actual: &[Type]) -> bool {
    match (expected, actual) {
        ([Type::Atom(e_name)], [Type::Atom(a_name)]) => e_name == a_name,
        ([Type::Atom(e_name), e_rest @ ..], [Type::Atom(a_name), a_rest @ ..]) => {
            e_name == a_name
                && e_rest.len() == a_rest.len()
                && e_rest
                    .iter()
                    .zip(a_rest.iter())
                    .all(|(x, y)| types_compatible(x, y))
        }
        _ => false,
    }
}

/// Reject a payload-less label type annotation (`Label(:ok)`).
///
/// `:ok` is now an Atom, so `Label(:ok)` is a contradiction: a Label requires
/// a payload. Callers should write `:ok` for the exact Atom, or
/// `Label(:ok, T)` for a payload label. The bare `Label` (broad union) and
/// the two-arg `Label(:name, T)` forms stay valid.
pub fn reject_payloadless_label_type(ty: &Type) -> Result<(), String> {
    match ty {
        Type::App(n, args) if n == "Label" => match args.as_slice() {
            [Type::Atom(name)] => Err(format!(
                "use :{name} instead of Label(:{name})"
            )),
            _ => Ok(()),
        },
        _ => Ok(()),
    }
}

/// Whether a static type denotes the `:error` label.
///
/// Matches the `err` sugar (`Label(:error, any)`) and both named forms
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

/// Resolve compile-time `Named` / `Self` annotations against known structs
/// and closed label sets.
///
/// `App` arguments are rewritten recursively so `List(Self)` and
/// `List(NamedStruct)` become `List(Struct(...))`. A PascalCase name that
/// matches a visible closed label set becomes `Type::ClosedLabelSet(name)`.
/// Constructor names are validated by [`validate_type_app`] after arguments
/// are rewritten.
pub fn resolve_type(
    ty: &mut Type,
    known_structs: &std::collections::HashSet<String>,
    known_closed_sets: &std::collections::HashSet<String>,
    self_struct: Option<&str>,
) -> Result<(), String> {
    match ty {
        Type::Named(name) => {
            let name = name.clone();
            if known_structs.contains(&name) {
                *ty = Type::Struct(name);
                Ok(())
            } else if known_closed_sets.contains(&name) {
                *ty = Type::ClosedLabelSet(name);
                Ok(())
            } else {
                Err(format!("Undefined type: {}", name))
            }
        }
        Type::SelfType => match self_struct {
            Some(name) => {
                *ty = Type::Struct(name.to_string());
                Ok(())
            }
            None => Err("`Self` is only valid in struct field type annotations".to_string()),
        },
        Type::App(name, args) => {
            for arg in &mut *args {
                resolve_type(arg, known_structs, known_closed_sets, self_struct)?;
            }
            validate_type_app(name, args)
        }
        _ => Ok(()),
    }
}

/// Builtin type constructors and their required arity / argument shape.
pub fn validate_type_app(name: &str, args: &[Type]) -> Result<(), String> {
    match name {
        "List" => {
            if args.len() == 1 {
                Ok(())
            } else {
                Err("List requires exactly one type argument".to_string())
            }
        }
        "Process" => {
            if args.len() == 1 {
                Ok(())
            } else {
                Err("Process requires exactly one type argument".to_string())
            }
        }
        "Label" => match args {
            [Type::Atom(_), _] => Ok(()),
            _ => Err(
                "Label application must be Label or Label(:name, T)".to_string(),
            ),
        },
        "Range" | "Buffer" | "RawPtr" | "Any" | "Self" => {
            if args.is_empty() {
                Ok(())
            } else {
                Err(format!("{name} does not take type arguments"))
            }
        }
        _ => Err(format!(
            "unknown type constructor `{name}`; builtin constructors are List(T), Process(T), Label(:name, T)"
        )),
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
    use std::collections::HashSet;

    #[test]
    fn tag_discriminants_match_known_tag_values() {
        assert_eq!(Type::Int.tag_discriminant(), Some(0));
        assert_eq!(Type::List.tag_discriminant(), Some(4));
        assert_eq!(Type::Buffer.tag_discriminant(), Some(11));
        assert_eq!(Type::Range.tag_discriminant(), Some(5));
        // Label is the surface union of tags 9/10: no static tag.
        assert_eq!(Type::Label.tag_discriminant(), None);
        assert_eq!(Type::Any.tag_discriminant(), None);
        assert_eq!(Type::Atom("ok".into()).tag_discriminant(), None);
        assert_eq!(
            Type::App("List".into(), vec![Type::Int]).tag_discriminant(),
            None
        );
        assert_eq!(
            Type::App("List".into(), vec![Type::Int]).runtime_shape(),
            Some(4)
        );
        assert_eq!(
            Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Str]).runtime_shape(),
            Some(10)
        );
        assert_eq!(
            Type::App("Process".into(), vec![Type::Int]).runtime_shape(),
            None
        );
        assert_eq!(Type::Param("T".into()).tag_discriminant(), None);
        assert_eq!(Type::from_tag_discriminant(4), Some(Type::List));
        assert_eq!(Type::AtomVal.tag_discriminant(), Some(9));
        assert_eq!(Type::from_tag_discriminant(9), Some(Type::AtomVal));
        assert_eq!(Type::from_tag_discriminant(10), Some(Type::Label));
        assert_eq!(Type::from_tag_discriminant(11), Some(Type::Buffer));
        assert_eq!(Type::RawPtr.tag_discriminant(), Some(12));
        assert_eq!(Type::from_tag_discriminant(12), Some(Type::RawPtr));
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
        let label_ok_i64 = Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::TypeI64]);
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

        // broad Label accepts payload-less atoms and payload labels
        assert!(types_compatible(&Type::Label, &Type::Atom("ok".into())));
        assert!(types_compatible(&Type::Label, &label_ok_int));
        assert!(types_compatible(&label_ok_int, &Type::Label));

        // exact label forms match by name only
        assert!(types_compatible(&label_ok, &label_ok));
        assert!(types_compatible(&label_ok_int, &label_ok_i64));
        assert!(!types_compatible(&label_ok, &label_err));

        // Label(:name, T) recurses into the payload
        assert!(types_compatible(&label_ok_int, &label_ok_i64));
        // Any payload is unconstrained, so Label(:ok, Int) ≡ Label(:ok, Any)
        assert!(types_compatible(&label_ok_int, &label_ok_any));
        assert!(!types_compatible(&label_ok_int, &label_ok));
        assert!(!types_compatible(&label_ok, &label_ok_any));

        // names must match
        assert!(!types_compatible(
            &Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Int]),
            &label_err
        ));

        // bare Int arg (name free) never matches Atom(name)
        assert!(!types_compatible(&label_free_int, &label_ok_int));
        assert!(!types_compatible(&label_ok_int, &label_free_int));
        assert!(!types_compatible(&label_int_str, &label_ok_int));

        // closed label sets and flat AtomVal are accepted by broad Label
        assert!(types_compatible(&Type::Label, &Type::ClosedLabelSet("Color".into())));
        assert!(types_compatible(&Type::Label, &Type::AtomVal));
    }

    #[test]
    fn types_compatible_closed_label_sets_by_name() {
        assert!(types_compatible(
            &Type::ClosedLabelSet("Color".into()),
            &Type::ClosedLabelSet("Color".into())
        ));
        assert!(!types_compatible(
            &Type::ClosedLabelSet("Color".into()),
            &Type::ClosedLabelSet("Status".into())
        ));
        assert!(!types_compatible(
            &Type::ClosedLabelSet("Color".into()),
            &Type::AtomVal
        ));
        // the old App("Atom", ...) bridge is gone
        assert!(!types_compatible(
            &Type::ClosedLabelSet("Color".into()),
            &Type::App("Atom".into(), vec![])
        ));
        assert!(!types_compatible(
            &Type::AtomVal,
            &Type::App("Atom".into(), vec![Type::Atom("ok".into())])
        ));
        assert!(!types_compatible(
            &Type::App("Atom".into(), vec![Type::Atom("ok".into())]),
            &Type::Atom("ok".into())
        ));
    }

    #[test]
    fn is_error_label_type_matches_err_sugar_and_named_forms() {
        let err_sugar = Type::App("Label".into(), vec![Type::Atom("error".into())]);
        let err_payload = Type::App("Label".into(), vec![Type::Atom("error".into()), Type::Str]);
        let ok_label = Type::App("Label".into(), vec![Type::Atom("ok".into())]);
        let ok_payload = Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Int]);

        assert!(is_error_label_type(&err_sugar));
        assert!(is_error_label_type(&err_payload));
        assert!(!is_error_label_type(&ok_label));
        assert!(!is_error_label_type(&ok_payload));
        assert!(!is_error_label_type(&Type::Label));
        assert!(!is_error_label_type(&Type::Any));
        assert!(!is_error_label_type(&Type::App(
            "Result".into(),
            vec![Type::Int, err_sugar.clone()]
        )));
    }

    #[test]
    fn resolves_named_self_and_nested_types() {
        let mut known = HashSet::new();
        known.insert("Node".to_string());
        known.insert("A".to_string());
        let mut closed = HashSet::new();
        closed.insert("ConnectionState".to_string());

        let mut self_ty = Type::SelfType;
        resolve_type(&mut self_ty, &known, &closed, Some("Node")).unwrap();
        assert_eq!(self_ty, Type::Struct("Node".into()));

        let mut list_self = Type::App("List".into(), vec![Type::SelfType]);
        resolve_type(&mut list_self, &known, &closed, Some("Node")).unwrap();
        assert_eq!(
            list_self,
            Type::App("List".into(), vec![Type::Struct("Node".into())])
        );

        let mut named = Type::Named("A".into());
        resolve_type(&mut named, &known, &closed, None).unwrap();
        assert_eq!(named, Type::Struct("A".into()));

        let mut set_named = Type::Named("ConnectionState".into());
        resolve_type(&mut set_named, &known, &closed, None).unwrap();
        assert_eq!(set_named, Type::ClosedLabelSet("ConnectionState".into()));

        let mut unknown = Type::Named("Nope".into());
        assert_eq!(
            resolve_type(&mut unknown, &known, &closed, None).unwrap_err(),
            "Undefined type: Nope"
        );

        let mut bad_self = Type::SelfType;
        assert_eq!(
            resolve_type(&mut bad_self, &known, &closed, None).unwrap_err(),
            "`Self` is only valid in struct field type annotations"
        );
    }

    #[test]
    fn display_renders_canonical_surface_spellings() {
        assert_eq!(Type::Int.to_string(), "i64");
        assert_eq!(Type::Float.to_string(), "f64");
        assert_eq!(Type::TypeI8.to_string(), "i8");
        assert_eq!(Type::TypeU64.to_string(), "u64");
        assert_eq!(Type::TypeF16.to_string(), "f16");
        assert_eq!(Type::TypeF64.to_string(), "f64");
        assert_eq!(Type::Bool.to_string(), "bool");
        assert_eq!(Type::Str.to_string(), "str");
        assert_eq!(Type::Unit.to_string(), "unit");
        assert_eq!(Type::Any.to_string(), "Any");
        assert_eq!(Type::List.to_string(), "List");
        assert_eq!(Type::Range.to_string(), "Range");
        assert_eq!(Type::Buffer.to_string(), "Buffer");
        assert_eq!(Type::RawPtr.to_string(), "RawPtr");
        assert_eq!(Type::Label.to_string(), "Label");
        assert_eq!(Type::AtomVal.to_string(), "Atom");
        assert_eq!(Type::Atom("ok".into()).to_string(), ":ok");
        assert_eq!(
            Type::App("List".into(), vec![Type::Int]).to_string(),
            "List(i64)"
        );
        assert_eq!(
            Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Str]).to_string(),
            "Label(:ok, str)"
        );
        assert_eq!(
            Type::App("Result".into(), vec![Type::Int, Type::Str]).to_string(),
            "Result(i64, str)"
        );
        assert_eq!(
            Type::App("Process".into(), vec![Type::Str]).to_string(),
            "Process(str)"
        );
        assert_eq!(Type::Param("T".into()).to_string(), "T");
        assert_eq!(Type::Struct("Job".into()).to_string(), "Job");
        assert_eq!(
            Type::ClosedLabelSet("ConnectionState".into()).to_string(),
            "ConnectionState"
        );
    }

    #[test]
    fn types_assignable_list_widening() {
        let list_int = Type::App("List".into(), vec![Type::Int]);
        let list_any = Type::App("List".into(), vec![Type::Any]);
        let list_str = Type::App("List".into(), vec![Type::Str]);
        assert!(types_assignable(&list_any, &list_int));
        assert!(!types_assignable(&list_int, &list_any));
        assert!(!types_assignable(&list_int, &list_str));
        assert!(types_assignable(&list_int, &Type::Any));
        assert_eq!(join_list_element_types(&[Type::Int, Type::Int]), Type::Int);
        assert_eq!(
            join_list_element_types(&[Type::Int, Type::Str]),
            Type::Any
        );
        assert_eq!(join_list_element_types(&[]), Type::Any);
        assert_eq!(
            process_result_type(&process_type(Type::Str)),
            Some(&Type::Str)
        );
        assert!(validate_type_app("List", &[Type::Int]).is_ok());
        assert!(validate_type_app("List", &[Type::Int, Type::Str]).is_err());
        assert!(validate_type_app("Process", &[Type::Int]).is_ok());
        assert!(validate_type_app("Range", &[Type::Int]).is_err());
        assert!(validate_type_app("Result", &[Type::Int]).is_err());
    }

    fn reject_payloadless_label_type_uses_atom_spelling() {
        let bad = Type::App("Label".into(), vec![Type::Atom("ok".into())]);
        assert_eq!(
            reject_payloadless_label_type(&bad).unwrap_err(),
            "use :ok instead of Label(:ok)"
        );
        let ok = Type::App("Label".into(), vec![Type::Atom("ok".into()), Type::Int]);
        assert!(reject_payloadless_label_type(&ok).is_ok());
        assert!(reject_payloadless_label_type(&Type::Label).is_ok());
        assert!(reject_payloadless_label_type(&Type::Atom("ok".into())).is_ok());
    }
}

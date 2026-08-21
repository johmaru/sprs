//! FunctionBuild contract resolution (Phase 1–3).
//!
//! Canonical surface: `function_build source name;`, `params(...)`,
//! `return_type(T)`, `visibility(pub|private)`, `type_param T;`,
//! `when COND { return_type(T); }`. Lowered onto existing `Function` fields
//! before prototype declaration / codegen. Call sites reuse `resolve_call_contract`.

use crate::front::ast::{FbCondition, FunctionBuild, FunctionBuildDirective, FunctionParam, Item};
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::type_helper::{self, types_assignable, types_compatible, Type};
use crate::front::parser::parse_only;
use crate::naming;
use std::collections::{HashMap, HashSet};

/// Common resolved function contract shared by inline `fn` headers and FunctionBuild.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFunctionSignature {
    pub params: Vec<FunctionParam>,
    pub ret_ty: Option<Type>,
    pub is_public: bool,
    /// Declared `type_param T;` names in source order. Empty for normal functions.
    pub type_params: Vec<String>,
    /// Conditional `when CONDITION { return_type(T); }` rules, in source order.
    pub when_rules: Vec<(FbCondition, Type)>,
}

#[derive(Debug, Clone)]
pub struct RegisteredFunctionBuild {
    pub ident: String,
    /// Visibility of the FunctionBuild declaration (`pub function_build`).
    pub is_public: bool,
    pub from_external: bool,
    pub signature: ResolvedFunctionSignature,
    pub span: Span,
    pub file: String,
}

#[derive(Debug, Default)]
pub struct FunctionBuildRegistry {
    pub builds: HashMap<String, RegisteredFunctionBuild>,
    /// Private builds seen in an external source (not imported).
    pub external_private: HashMap<String, Span>,
}

fn semantic(
    file: &str,
    span: Span,
    number: u32,
    message: String,
    help: Option<String>,
) -> SprsError {
    SprsError::Semantic {
        code: ErrorCode {
            category: ErrorCategory::Semantic,
            number,
        },
        location: Location::new(file.to_string(), span),
        message,
        help,
    }
}

/// Collapse FunctionBuild directives into a single static signature.
/// Duplicate directives are errors (no last-wins / merge).
pub fn resolve_function_build_signature(
    fb: &FunctionBuild,
    file: &str,
) -> Result<ResolvedFunctionSignature, SprsError> {
    let mut params: Option<(Vec<FunctionParam>, Span)> = None;
    let mut ret_ty: Option<(Type, Span)> = None;
    let mut visibility: Option<(bool, Span)> = None;
    let mut type_params: Vec<String> = Vec::new();
    let mut when_rules: Vec<(FbCondition, Type)> = Vec::new();

    for directive in &fb.directives {
        match directive {
            FunctionBuildDirective::Params { params: args, span } => {
                if let Some((_, prev)) = params {
                    return Err(duplicate_directive_error(file, *span, prev, "params"));
                }
                params = Some((args.clone(), *span));
            }
            FunctionBuildDirective::ReturnType { ty, span } => {
                if let Some((_, prev)) = ret_ty {
                    return Err(duplicate_directive_error(file, *span, prev, "return_type"));
                }
                ret_ty = Some((ty.clone(), *span));
            }
            FunctionBuildDirective::Visibility { is_public, span } => {
                if let Some((_, prev)) = visibility {
                    return Err(duplicate_directive_error(file, *span, prev, "visibility"));
                }
                visibility = Some((*is_public, *span));
            }
            FunctionBuildDirective::TypeParam { ident, span } => {
                if type_params.iter().any(|existing| existing == ident) {
                    return Err(semantic(
                        file,
                        *span,
                        19,
                        format!("duplicate type parameter `{ident}`"),
                        Some("each `type_param` name may appear at most once".to_string()),
                    ));
                }
                type_params.push(ident.clone());
            }
            FunctionBuildDirective::When { condition, ret_ty: when_ty, span } => {
                let _ = span;
                when_rules.push((condition.clone(), when_ty.clone()));
            }
        }
    }

    Ok(ResolvedFunctionSignature {
        params: params.map(|(p, _)| p).unwrap_or_default(),
        ret_ty: ret_ty.map(|(ty, _)| ty),
        is_public: visibility.map(|(v, _)| v).unwrap_or(false),
        type_params,
        when_rules,
    })
}

fn duplicate_directive_error(file: &str, span: Span, _prev: Span, name: &str) -> SprsError {
    semantic(
        file,
        span,
        20,
        format!("duplicate FunctionBuild directive {name}"),
        Some(
            "each params / return_type / visibility directive may appear at most once".to_string(),
        ),
    )
}

pub fn function_build_source_directive(
    items: &[Item],
    file: &str,
) -> Result<Option<(String, Span)>, SprsError> {
    let mut found: Option<(String, Span)> = None;
    for item in items {
        if let Item::FunctionBuildSource { target, span } = item {
            if found.is_some() {
                return Err(semantic(
                    file,
                    *span,
                    23,
                    "multiple `function_build source` directives in one file".to_string(),
                    Some("a source file may specify at most one FunctionBuild source".to_string()),
                ));
            }
            found = Some((target.clone(), *span));
        }
    }
    Ok(found)
}

pub fn collect_local_function_builds(
    items: &[Item],
    file: &str,
    from_external: bool,
) -> Result<Vec<RegisteredFunctionBuild>, SprsError> {
    let mut seen: HashMap<String, Span> = HashMap::new();
    let mut out = Vec::new();
    for item in items {
        let Item::FunctionBuildItem(fb) = item else {
            continue;
        };
        if let Some(&prev) = seen.get(&fb.ident) {
            let _ = prev;
            return Err(semantic(
                file,
                fb.span,
                19,
                format!("duplicate FunctionBuild `{}`", fb.ident),
                Some(
                    "FunctionBuild names must be unique in the FunctionBuild namespace".to_string(),
                ),
            ));
        }
        seen.insert(fb.ident.clone(), fb.span);
        let signature = resolve_function_build_signature(fb, file)?;
        out.push(RegisteredFunctionBuild {
            ident: fb.ident.clone(),
            is_public: fb.is_public,
            from_external,
            signature,
            span: fb.span,
            file: file.to_string(),
        });
    }
    Ok(out)
}

pub fn insert_builds(
    registry: &mut FunctionBuildRegistry,
    builds: Vec<RegisteredFunctionBuild>,
) -> Result<(), SprsError> {
    for build in builds {
        if let Some(existing) = registry.builds.get(&build.ident) {
            return Err(semantic(
                &build.file,
                build.span,
                19,
                format!("duplicate FunctionBuild `{}`", build.ident),
                Some(format!("previously defined in {}", existing.file)),
            ));
        }
        registry.builds.insert(build.ident.clone(), build);
    }
    Ok(())
}

pub fn known_structs_from_items(items: &[Item]) -> HashSet<String> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::StructItem(struct_item) => Some(struct_item.ident.clone()),
            _ => None,
        })
        .collect()
}

/// Builtin type constructor names that cannot be shadowed by a type parameter.
const BUILTIN_TYPE_NAMES: &[&str] =
    &["Any", "List", "Label", "Process", "Range", "Buffer", "RawPtr", "Self"];

/// Convert a FunctionBuild type annotation in place.
///
/// A PascalCase `Named` that matches a declared `type_param` becomes
/// `Type::Param`; anything else goes through the regular struct / closed
/// label set resolution (unknown names are `Undefined type` errors, which
/// covers undeclared type parameter references).
fn convert_fb_type(
    ty: &mut Type,
    declared: &HashSet<&String>,
    known_structs: &HashSet<String>,
    known_closed_sets: &HashSet<String>,
) -> Result<(), String> {
    match ty {
        Type::Named(name) => {
            let name_clone = name.clone();
            if declared.contains(&name_clone) {
                *ty = Type::Param(name_clone);
                Ok(())
            } else {
                type_helper::resolve_type(ty, known_structs, known_closed_sets, None)
            }
        }
        Type::App(_, args) => {
            for arg in args {
                convert_fb_type(arg, declared, known_structs, known_closed_sets)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Convert every `FbCondition::Type` operand inside a `when` condition.
fn convert_condition_types(
    cond: &mut FbCondition,
    declared: &HashSet<&String>,
    known_structs: &HashSet<String>,
    known_closed_sets: &HashSet<String>,
) -> Result<(), String> {
    match cond {
        FbCondition::Type(ty) => convert_fb_type(ty, declared, known_structs, known_closed_sets),
        FbCondition::Bool(_) => Ok(()),
        FbCondition::Is { lhs, rhs } | FbCondition::Neq { lhs, rhs } => {
            convert_condition_types(lhs, declared, known_structs, known_closed_sets)?;
            convert_condition_types(rhs, declared, known_structs, known_closed_sets)
        }
        FbCondition::And { lhs, rhs } | FbCondition::Or { lhs, rhs } => {
            convert_condition_types(lhs, declared, known_structs, known_closed_sets)?;
            convert_condition_types(rhs, declared, known_structs, known_closed_sets)
        }
        FbCondition::Not { inner } => {
            convert_condition_types(inner, declared, known_structs, known_closed_sets)
        }
    }
}

pub fn resolve_function_build_types(
    items: &mut [Item],
    known_structs: &HashSet<String>,
    known_closed_sets: &HashSet<String>,
    path: &str,
) -> Result<(), SprsError> {
    for item in items.iter_mut() {
        let Item::FunctionBuildItem(fb) = item else {
            continue;
        };

        // Collect declared type parameters, rejecting duplicates and
        // collisions with builtin constructors / visible structs / closed sets.
        let mut declared: Vec<String> = Vec::new();
        for directive in &fb.directives {
            if let FunctionBuildDirective::TypeParam { ident, span } = directive {
                if declared.iter().any(|existing| existing == ident) {
                    return Err(semantic(
                        path,
                        *span,
                        19,
                        format!("duplicate type parameter `{ident}`"),
                        Some("each `type_param` name may appear at most once".to_string()),
                    ));
                }
                if BUILTIN_TYPE_NAMES.contains(&ident.as_str())
                    || known_structs.contains(ident)
                    || known_closed_sets.contains(ident)
                {
                    return Err(semantic(
                        path,
                        *span,
                        19,
                        format!(
                            "type parameter `{ident}` collides with an existing type name"
                        ),
                        Some(
                            "choose a name that does not shadow a builtin type, struct, or closed label set"
                                .to_string(),
                        ),
                    ));
                }
                declared.push(ident.clone());
            }
        }
        let declared_set: HashSet<&String> = declared.iter().collect();

        for directive in &mut fb.directives {
            match directive {
                FunctionBuildDirective::Params { params, .. } => {
                    for param in params {
                        if let Some(annot) = &mut param.ty {
                            convert_fb_type(&mut annot.ty, &declared_set, known_structs, known_closed_sets)
                                .map_err(|message| semantic(path, param.span, 11, message, None))?;
                        }
                    }
                }
                FunctionBuildDirective::ReturnType { ty, span } => {
                    convert_fb_type(ty, &declared_set, known_structs, known_closed_sets)
                        .map_err(|message| semantic(path, *span, 11, message, None))?;
                }
                FunctionBuildDirective::When { condition, ret_ty, span } => {
                    convert_fb_type(ret_ty, &declared_set, known_structs, known_closed_sets)
                        .map_err(|message| semantic(path, *span, 11, message, None))?;
                    convert_condition_types(condition, &declared_set, known_structs, known_closed_sets)
                        .map_err(|message| semantic(path, *span, 11, message, None))?;
                }
                FunctionBuildDirective::Visibility { .. }
                | FunctionBuildDirective::TypeParam { .. } => {}
            }
        }
    }
    Ok(())
}

pub fn lower_functions_with_builds(
    items: &mut [Item],
    registry: &FunctionBuildRegistry,
    file: &str,
) -> Result<(), SprsError> {
    for item in items.iter_mut() {
        let Item::FunctionItem(func) = item else {
            continue;
        };
        let Some(build_name) = func.build_ref.clone() else {
            continue;
        };
        if let Some(build) = registry.builds.get(&build_name) {
            if build.from_external && !build.is_public {
                return Err(semantic(
                    file,
                    func.build_ref_span,
                    22,
                    format!(
                        "FunctionBuild `{build_name}` is private and cannot be used from an external source"
                    ),
                    None,
                ));
            }
            func.params = build.signature.params.clone();
            func.ret_ty = build.signature.ret_ty.clone();
            func.is_public = build.signature.is_public;
            continue;
        }
        if registry.external_private.contains_key(&build_name) {
            return Err(semantic(
                file,
                func.build_ref_span,
                22,
                format!(
                    "FunctionBuild `{build_name}` is private and cannot be used from an external source"
                ),
                None,
            ));
        }
        return Err(semantic(
            file,
            func.build_ref_span,
            18,
            format!("undefined FunctionBuild `{build_name}`"),
            None,
        ));
    }
    Ok(())
}

/// Errors from FunctionBuild call-contract resolution. Callers map these onto
/// `SprsError` with the appropriate code (arity -> SEM-016, type conflicts and
/// unresolved type parameters -> type/semantic errors).
#[derive(Debug, Clone, PartialEq)]
pub enum CallContractError {
    Arity { expected: usize, actual: usize },
    /// Type unification conflict on a type parameter or a param pattern.
    TypeConflict { message: String },
    /// A `Type::Param` stayed unbound or bound to `Any` after unification.
    UnresolvedTypeParam { name: String },
    /// `neq` / condition evaluation needs concrete types.
    NotConcrete { message: String },
    /// Two or more `when` rules matched.
    MultipleMatches,
}

/// Unify a parameter pattern with an actual argument type.
///
/// `Type::Param` binds (weak: a later concrete type overwrites an earlier
/// `Any`; a conflicting concrete type is a type error). `Any` patterns always
/// match. `App` recurses; everything else falls back to `types_compatible`.
fn unify(
    pattern: &Type,
    actual: &Type,
    bindings: &mut HashMap<String, Type>,
) -> Result<(), CallContractError> {
    if let Type::Param(name) = pattern {
        match bindings.get(name) {
            None => {
                bindings.insert(name.clone(), actual.clone());
                Ok(())
            }
            Some(previous) => {
                if *previous == Type::Any {
                    if *actual != Type::Any {
                        bindings.insert(name.clone(), actual.clone());
                    }
                    Ok(())
                } else if *actual == Type::Any || types_compatible(previous, actual) {
                    Ok(())
                } else {
                    Err(CallContractError::TypeConflict {
                        message: format!(
                            "type parameter `{name}` was already resolved to `{previous}`, but the argument has type `{actual}`"
                        ),
                    })
                }
            }
        }
    } else if matches!(pattern, Type::Any) {
        Ok(())
    } else if let (Type::App(n1, a1), Type::App(n2, a2)) = (pattern, actual) {
        if n1 == "List" && n2 == "List" {
            match (type_helper::list_element(pattern), type_helper::list_element(actual)) {
                (Some(Type::Any), _) => return Ok(()),
                (Some(p), Some(Type::Any)) if !matches!(p, Type::Param(_)) => {
                    return Err(CallContractError::TypeConflict {
                        message: format!("expected `{pattern}`, found `{actual}`"),
                    });
                }
                (Some(p), Some(a)) => return unify(p, a, bindings),
                _ => {
                    return Err(CallContractError::TypeConflict {
                        message: format!("expected `{pattern}`, found `{actual}`"),
                    });
                }
            }
        }
        if n1 != n2 || a1.len() != a2.len() {
            return Err(CallContractError::TypeConflict {
                message: format!("expected `{pattern}`, found `{actual}`"),
            });
        }
        for (pat_arg, act_arg) in a1.iter().zip(a2.iter()) {
            unify(pat_arg, act_arg, bindings)?;
        }
        Ok(())
    } else if types_assignable(pattern, actual) {
        Ok(())
    } else {
        Err(CallContractError::TypeConflict {
            message: format!("expected `{pattern}`, found `{actual}`"),
        })
    }
}

/// Substitute concrete type parameter bindings into a type.
fn substitute_type(
    ty: &Type,
    bindings: &HashMap<String, Type>,
) -> Result<Type, CallContractError> {
    match ty {
        Type::Param(name) => bindings.get(name).cloned().ok_or_else(|| {
            CallContractError::UnresolvedTypeParam {
                name: name.clone(),
            }
        }),
        Type::App(name, args) => {
            let mut substituted = Vec::with_capacity(args.len());
            for arg in args {
                substituted.push(substitute_type(arg, bindings)?);
            }
            Ok(Type::App(name.clone(), substituted))
        }
        other => Ok(other.clone()),
    }
}

/// Evaluate a `when` condition against the concrete type bindings.
///
/// `is` uses canonical type compatibility after substitution; `neq` requires
/// both sides concrete; `and`/`or`/`not` short-circuit. A bare type operand
/// is true when it resolves to a concrete (non-`Any`) type.
fn eval_condition(
    cond: &FbCondition,
    bindings: &HashMap<String, Type>,
) -> Result<bool, CallContractError> {
    match cond {
        FbCondition::Bool(b) => Ok(*b),
        FbCondition::Type(ty) => Ok(substitute_type(ty, bindings)? != Type::Any),
        FbCondition::Is { lhs, rhs } => {
            let left = resolve_cond_type(lhs, bindings)?;
            let right = resolve_cond_type(rhs, bindings)?;
            Ok(types_compatible(&left, &right))
        }
        FbCondition::Neq { lhs, rhs } => {
            let left = resolve_cond_type(lhs, bindings)?;
            let right = resolve_cond_type(rhs, bindings)?;
            if left == Type::Any || right == Type::Any {
                return Err(CallContractError::NotConcrete {
                    message: "`neq` requires concrete types on both sides".to_string(),
                });
            }
            Ok(!types_compatible(&left, &right))
        }
        FbCondition::And { lhs, rhs } => {
            Ok(eval_condition(lhs, bindings)? && eval_condition(rhs, bindings)?)
        }
        FbCondition::Or { lhs, rhs } => {
            Ok(eval_condition(lhs, bindings)? || eval_condition(rhs, bindings)?)
        }
        FbCondition::Not { inner } => Ok(!eval_condition(inner, bindings)?),
    }
}

/// Resolve a condition operand (`FbCondition::Type`) to its substituted type.
fn resolve_cond_type(
    cond: &FbCondition,
    bindings: &HashMap<String, Type>,
) -> Result<Type, CallContractError> {
    match cond {
        FbCondition::Type(ty) => substitute_type(ty, bindings),
        other => Err(CallContractError::NotConcrete {
            message: format!("condition operand must be a type, got {other:?}"),
        }),
    }
}

/// Resolve a FunctionBuild call contract against actual argument types.
///
/// Steps: (1) arity, (2) recursive unification of parameter patterns with
/// actual types (weak `Any` bindings), (3) every bound type parameter is
/// concrete, (4) `when` condition evaluation, (5) return type substitution.
///
/// Returns the substituted return type: `Ok(None)` means unannotated / `Any`.
pub fn resolve_call_contract(
    sig: &ResolvedFunctionSignature,
    actuals: &[Type],
) -> Result<Option<Type>, CallContractError> {
    if sig.params.len() != actuals.len() {
        return Err(CallContractError::Arity {
            expected: sig.params.len(),
            actual: actuals.len(),
        });
    }

    let mut bindings: HashMap<String, Type> = HashMap::new();
    for (param, actual) in sig.params.iter().zip(actuals.iter()) {
        if let Some(annot) = &param.ty {
            unify(&annot.ty, actual, &mut bindings)?;
        }
    }
    // A type parameter bound only to `Any` was never concretely resolved.
    for (name, bound) in &bindings {
        if *bound == Type::Any {
            return Err(CallContractError::UnresolvedTypeParam {
                name: name.clone(),
            });
        }
    }

    let mut matched: Option<Type> = None;
    for (cond, ret_ty) in &sig.when_rules {
        if eval_condition(cond, &bindings)? {
            if matched.is_some() {
                return Err(CallContractError::MultipleMatches);
            }
            matched = Some(substitute_type(ret_ty, &bindings)?);
        }
    }
    if let Some(ty) = matched {
        return Ok(Some(ty));
    }

    match &sig.ret_ty {
        Some(ty) => Ok(Some(substitute_type(ty, &bindings)?)),
        None => Ok(None),
    }
}

/// Load a FunctionBuild declaration source (not a runtime module).
///
/// Follows nested `function_build source` only for cycle detection.
/// Only the directly named source's public FunctionBuild declarations are returned.
pub fn load_function_build_source(
    source_name: &str,
    request_span: Span,
    request_file: &str,
    source_path: &str,
    stack: &mut Vec<String>,
) -> Result<(Vec<Item>, String), SprsError> {
    if stack.iter().any(|name| name == source_name) {
        let mut cycle = stack.clone();
        cycle.push(source_name.to_string());
        return Err(semantic(
            request_file,
            request_span,
            24,
            format!("circular FunctionBuild source: {}", cycle.join(" -> ")),
            None,
        ));
    }

    let path = format!("{}/{}{}", source_path, source_name, naming::SOURCE_EXT);
    let source = std::fs::read_to_string(&path).map_err(|load_error| {
        semantic(
            request_file,
            request_span,
            10,
            format!("Failed to read module file {}: {}", path, load_error),
            None,
        )
    })?;

    stack.push(source_name.to_string());
    let mut items = parse_only(&source, &path)?;
    if let Some((nested, nested_span)) = function_build_source_directive(&items, &path)? {
        // Recurse for cycle detection; nested public builds are not re-exported.
        let _ = load_function_build_source(&nested, nested_span, &path, source_path, stack)?;
    }
    stack.pop();

    let known = known_structs_from_items(&items);
    let mut known_closed_sets: HashSet<String> = HashSet::new();
    for item in &items {
        if let Item::ClosedLabelSetItem(set) = item {
            known_closed_sets.insert(set.ident.clone());
        }
    }
    resolve_function_build_types(&mut items, &known, &known_closed_sets, &path)?;
    Ok((items, path))
}

pub fn import_public_builds_from_source(
    items: &[Item],
    file: &str,
    registry: &mut FunctionBuildRegistry,
) -> Result<(), SprsError> {
    let collected = collect_local_function_builds(items, file, true)?;
    for build in collected {
        if build.is_public {
            insert_builds(registry, vec![build])?;
        } else {
            registry
                .external_private
                .insert(build.ident.clone(), build.span);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::parser::parse_only;

    fn parse(src: &str) -> Vec<Item> {
        parse_only(src, "test.sprs").expect("parse")
    }

    #[test]
    fn resolves_basic_function_build_signature() {
        let items = parse(
            r#"
function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
    visibility(pub);
}
"#,
        );
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0].ident, "lhs");
        assert_eq!(sig.ret_ty, Some(Type::TypeI64));
        assert!(sig.is_public);
        assert!(fb.is_public == false);
    }

    #[test]
    fn defaults_when_directives_omitted() {
        let items = parse("function_build Empty {}\n");
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        assert!(sig.params.is_empty());
        assert_eq!(sig.ret_ty, None);
        assert!(!sig.is_public);
        assert!(sig.type_params.is_empty());
        assert!(sig.when_rules.is_empty());
    }

    #[test]
    fn rejects_duplicate_directives() {
        let items = parse(
            r#"
function_build Bad {
    return_type(i64);
    return_type(str);
}
"#,
        );
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let err = resolve_function_build_signature(fb, "test.sprs").unwrap_err();
        match err {
            SprsError::Semantic {
                code,
                location,
                message,
                ..
            } => {
                assert_eq!(code.as_string(), "SPRS-SEM-020");
                assert!(message.contains("return_type"));
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lowers_function_from_build() {
        let mut items = parse(
            r#"
fn add use AddBuild {
    return lhs + rhs;
}
function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
    visibility(pub);
}
"#,
        );
        let builds = collect_local_function_builds(&items, "test.sprs", false).unwrap();
        let mut registry = FunctionBuildRegistry::default();
        insert_builds(&mut registry, builds).unwrap();
        lower_functions_with_builds(&mut items, &registry, "test.sprs").unwrap();
        let Item::FunctionItem(func) = &items[0] else {
            panic!("expected fn");
        };
        assert_eq!(func.params.len(), 2);
        assert_eq!(func.params[0].ident, "lhs");
        assert_eq!(func.ret_ty, Some(Type::TypeI64));
        assert!(func.is_public);
        assert_eq!(func.build_ref.as_deref(), Some("AddBuild"));
    }

    #[test]
    fn undefined_build_is_semantic_error() {
        let mut items = parse("fn foo use Missing {}\n");
        let registry = FunctionBuildRegistry::default();
        let err = lower_functions_with_builds(&mut items, &registry, "test.sprs").unwrap_err();
        match err {
            SprsError::Semantic {
                code,
                message,
                location,
                ..
            } => {
                assert_eq!(code.as_string(), "SPRS-SEM-018");
                assert!(message.contains("Missing"));
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn duplicate_local_builds_are_rejected() {
        let items = parse("function_build A {}\nfunction_build A {}\n");
        let err = collect_local_function_builds(&items, "test.sprs", false).unwrap_err();
        match err {
            SprsError::Semantic { code, location, .. } => {
                assert_eq!(code.as_string(), "SPRS-SEM-019");
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn multiple_source_directives_are_rejected() {
        let items = parse("function_build source a;\nfunction_build source b;\n");
        let err = function_build_source_directive(&items, "test.sprs").unwrap_err();
        match err {
            SprsError::Semantic { code, location, .. } => {
                assert_eq!(code.as_string(), "SPRS-SEM-023");
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn private_external_build_is_rejected() {
        let mut items = parse("fn foo use InternalBuild {}\n");
        let mut registry = FunctionBuildRegistry::default();
        registry
            .external_private
            .insert("InternalBuild".into(), Span::DUMMY);
        let err = lower_functions_with_builds(&mut items, &registry, "main.sprs").unwrap_err();
        match err {
            SprsError::Semantic { code, message, .. } => {
                assert_eq!(code.as_string(), "SPRS-SEM-022");
                assert!(message.contains("private"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reusable_build_lowers_two_functions() {
        let mut items = parse(
            r#"
function_build UnaryI64 {
    params(value >> i64);
    return_type(i64);
}
fn foo use UnaryI64 { return value; }
fn bar use UnaryI64 { return value; }
"#,
        );
        let builds = collect_local_function_builds(&items, "test.sprs", false).unwrap();
        let mut registry = FunctionBuildRegistry::default();
        insert_builds(&mut registry, builds).unwrap();
        lower_functions_with_builds(&mut items, &registry, "test.sprs").unwrap();
        for item in &items[1..] {
            let Item::FunctionItem(func) = item else {
                panic!("expected fn");
            };
            assert_eq!(func.params[0].ident, "value");
            assert_eq!(func.ret_ty, Some(Type::TypeI64));
        }
    }

    #[test]
    fn circular_source_is_rejected() {
        let dir = std::env::temp_dir().join(format!("sprs-fb-cycle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.sprs"), "function_build source b;\n").unwrap();
        std::fs::write(dir.join("b.sprs"), "function_build source a;\n").unwrap();
        let mut stack = vec!["root".to_string()];
        let err = load_function_build_source(
            "a",
            Span::new(1, 2),
            "root.sprs",
            dir.to_str().unwrap(),
            &mut stack,
        )
        .unwrap_err();
        match err {
            SprsError::Semantic {
                code,
                message,
                location,
                ..
            } => {
                assert_eq!(code.as_string(), "SPRS-SEM-024");
                assert!(message.contains("circular"));
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn external_public_import_skips_private_and_functions() {
        let src = r#"
pub function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
}
function_build InternalBuild {
    return_type(str);
}
fn helper() {}
"#;
        let items = parse(src);
        let mut registry = FunctionBuildRegistry::default();
        import_public_builds_from_source(&items, "contracts.sprs", &mut registry).unwrap();
        assert!(registry.builds.contains_key("AddBuild"));
        assert!(!registry.builds.contains_key("InternalBuild"));
        assert!(registry.external_private.contains_key("InternalBuild"));
    }

    #[test]
    fn local_and_external_duplicate_is_rejected() {
        let ext = parse("pub function_build Foo {}\n");
        let mut registry = FunctionBuildRegistry::default();
        import_public_builds_from_source(&ext, "contracts.sprs", &mut registry).unwrap();
        let local = parse("function_build Foo {}\n");
        let builds = collect_local_function_builds(&local, "main.sprs", false).unwrap();
        let err = insert_builds(&mut registry, builds).unwrap_err();
        match err {
            SprsError::Semantic { code, location, .. } => {
                assert_eq!(code.as_string(), "SPRS-SEM-019");
                assert_ne!(location.span, Span::DUMMY);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn function_and_build_may_share_a_name() {
        let mut items = parse("function_build Foo {}\nfn foo use Foo {}\n");
        let builds = collect_local_function_builds(&items, "test.sprs", false).unwrap();
        let mut registry = FunctionBuildRegistry::default();
        insert_builds(&mut registry, builds).unwrap();
        lower_functions_with_builds(&mut items, &registry, "test.sprs").unwrap();
        assert!(matches!(&items[1], Item::FunctionItem(func) if func.ident == "foo"));
    }

    #[test]
    fn resolves_named_struct_types_in_function_build() {
        let mut items = parse(
            r#"
struct Job { id >> i64 }
function_build JobFn {
    params(job >> Job);
    return_type(Job);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[1] else {
            panic!("expected function_build");
        };
        match &fb.directives[0] {
            FunctionBuildDirective::Params { params, .. } => {
                assert_eq!(
                    params[0].ty.as_ref().map(|annot| &annot.ty),
                    Some(&Type::Struct("Job".into()))
                );
            }
            other => panic!("{other:?}"),
        }
        match &fb.directives[1] {
            FunctionBuildDirective::ReturnType { ty, .. } => {
                assert_eq!(ty, &Type::Struct("Job".into()));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn converts_declared_type_params_to_param_types() {
        let mut items = parse(
            r#"
function_build Identity {
    type_param T;
    params(value >> T);
    return_type(T);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        assert_eq!(sig.type_params, vec!["T".to_string()]);
        assert_eq!(
            sig.params[0].ty.as_ref().map(|annot| &annot.ty),
            Some(&Type::Param("T".into()))
        );
        assert_eq!(sig.ret_ty, Some(Type::Param("T".into())));
    }

    #[test]
    fn recursive_list_of_type_param_binds() {
        let mut items = parse(
            r#"
function_build WrapList {
    type_param T;
    params(xs >> List(T));
    return_type(T);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        assert_eq!(
            sig.params[0].ty.as_ref().map(|annot| &annot.ty),
            Some(&Type::App(
                "List".into(),
                vec![Type::Param("T".into())]
            ))
        );
        // List(i64) resolves T to i64; the return type substitutes to i64.
        let resolved = resolve_call_contract(&sig, &[Type::App("List".into(), vec![Type::Int])])
            .unwrap();
        assert_eq!(resolved, Some(Type::Int));
    }

    #[test]
    fn identity_build_resolves_for_i64_and_str() {
        let mut items = parse(
            r#"
function_build Identity {
    type_param T;
    params(value >> T);
    return_type(T);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Int]).unwrap(),
            Some(Type::Int)
        );
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Str]).unwrap(),
            Some(Type::Str)
        );
    }

    #[test]
    fn same_param_conflict_is_type_error() {
        let mut items = parse(
            r#"
function_build Same {
    type_param T;
    params(left >> T, right >> T);
    return_type(T);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        let err = resolve_call_contract(&sig, &[Type::Int, Type::Str]).unwrap_err();
        match err {
            CallContractError::TypeConflict { message } => {
                assert!(message.contains("`T`"), "unexpected: {message}");
            }
            other => panic!("expected TypeConflict, got {other:?}"),
        }
        // consistent bindings succeed
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Int, Type::TypeI64]).unwrap(),
            Some(Type::Int)
        );
    }

    #[test]
    fn unresolved_param_reference_is_error() {
        let mut items = parse(
            r#"
function_build Unbound {
    type_param T;
    return_type(T);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        let err = resolve_call_contract(&sig, &[]).unwrap_err();
        match err {
            CallContractError::UnresolvedTypeParam { name } => assert_eq!(name, "T"),
            other => panic!("expected UnresolvedTypeParam, got {other:?}"),
        }
    }

    #[test]
    fn any_binding_is_weak_and_refined_later() {
        let mut items = parse(
            r#"
function_build Pair {
    type_param T;
    params(first >> T, second >> T);
    return_type(T);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        // First arg is Any (weak), second refines T to i64.
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Any, Type::Int]).unwrap(),
            Some(Type::Int)
        );
    }

    #[test]
    fn when_is_and_neq_select_return_type() {
        let mut items = parse(
            r#"
function_build Pick {
    type_param T;
    params(value >> T);
    return_type(T);
    when T is i64 { return_type(i64); }
    when T is str { return_type(str); }
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        assert_eq!(sig.when_rules.len(), 2);
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Int]).unwrap(),
            Some(Type::TypeI64)
        );
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Str]).unwrap(),
            Some(Type::Str)
        );
        // bool matches neither rule; falls back to unconditional return_type(T).
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Bool]).unwrap(),
            Some(Type::Bool)
        );
    }

    #[test]
    fn when_neq_and_and_or_not_evaluate() {
        let mut items = parse(
            r#"
function_build Cond {
    type_param T;
    params(value >> T);
    return_type(T);
    when T neq i64 and not (T is str) { return_type(i64); }
    when T is i64 or T is i64 { return_type(str); }
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        // bool: T neq i64 (true) and not (T is str) (true) -> i64
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Bool]).unwrap(),
            Some(Type::TypeI64)
        );
        // i64: first rule false (neq), second rule true (is) -> str
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Int]).unwrap(),
            Some(Type::Str)
        );
    }

    #[test]
    fn no_match_falls_back_to_unconditional_return_type() {
        let mut items = parse(
            r#"
function_build Fallback {
    type_param T;
    params(value >> T);
    return_type(i64);
    when T is str { return_type(str); }
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Int]).unwrap(),
            Some(Type::TypeI64)
        );
        assert_eq!(
            resolve_call_contract(&sig, &[Type::Str]).unwrap(),
            Some(Type::Str)
        );
    }

    #[test]
    fn multiple_when_matches_is_conflict() {
        let mut items = parse(
            r#"
function_build Ambiguous {
    type_param T;
    params(value >> T);
    return_type(T);
    when T is i64 { return_type(i64); }
    when T is i64 { return_type(i64); }
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[0] else {
            panic!("expected function_build");
        };
        let sig = resolve_function_build_signature(fb, "test.sprs").unwrap();
        let err = resolve_call_contract(&sig, &[Type::Int]).unwrap_err();
        assert_eq!(err, CallContractError::MultipleMatches);
    }

    #[test]
    fn duplicate_type_param_is_rejected() {
        let mut items = parse(
            r#"
function_build Dup {
    type_param T;
    type_param T;
    params(value >> T);
    return_type(T);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        let err =
            resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs")
                .unwrap_err();
        match err {
            SprsError::Semantic { code, message, .. } => {
                assert_eq!(code.as_string(), "SPRS-SEM-019");
                assert!(message.contains("T"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn type_param_collision_with_struct_is_rejected() {
        let mut items = parse(
            r#"
struct Job { id >> i64 }
function_build Bad {
    type_param Job;
    params(value >> Job);
    return_type(Job);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        let err =
            resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs")
                .unwrap_err();
        match err {
            SprsError::Semantic { code, message, .. } => {
                assert_eq!(code.as_string(), "SPRS-SEM-019");
                assert!(message.contains("collides"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn undeclared_type_param_reference_is_error() {
        let mut items = parse(
            r#"
function_build Bad {
    params(value >> Missing);
    return_type(Missing);
}
"#,
        );
        let known = known_structs_from_items(&items);
        let known_closed_sets = HashSet::new();
        let err =
            resolve_function_build_types(&mut items, &known, &known_closed_sets, "test.sprs")
                .unwrap_err();
        match err {
            SprsError::Semantic { code, message, .. } => {
                assert_eq!(code.as_string(), "SPRS-SEM-011");
                assert!(message.contains("Missing"));
            }
            other => panic!("{other:?}"),
        }
    }
}

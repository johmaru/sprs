//! Static FunctionBuild contract resolution (Phase 1).
//!
//! FunctionBuild syntax is lowered onto the existing `Function` fields
//! (`params`, `ret_ty`, `is_public`) before prototype declaration / codegen.
//! Phase 2 (`fbtype` / unification / `fbif`) is intentionally not implemented.

use crate::front::ast::{FunctionBuild, FunctionBuildDirective, FunctionParam, Item};
use crate::front::error::{ErrorCategory, ErrorCode, Location, SprsError};
use crate::front::span::Span;
use crate::front::type_helper::{self, Type};
use crate::llvm::parser::parse_only;
use crate::naming;
use std::collections::{HashMap, HashSet};

/// Common resolved function contract shared by inline `fn` headers and FunctionBuild.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFunctionSignature {
    pub params: Vec<FunctionParam>,
    pub ret_ty: Option<Type>,
    pub is_public: bool,
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

    for directive in &fb.directives {
        match directive {
            FunctionBuildDirective::Args { params: args, span } => {
                if let Some((_, prev)) = params {
                    return Err(duplicate_directive_error(file, *span, prev, "FbArgs"));
                }
                params = Some((args.clone(), *span));
            }
            FunctionBuildDirective::RetTy { ty, span } => {
                if let Some((_, prev)) = ret_ty {
                    return Err(duplicate_directive_error(file, *span, prev, "FbRetTy"));
                }
                ret_ty = Some((ty.clone(), *span));
            }
            FunctionBuildDirective::Visibility { is_public, span } => {
                if let Some((_, prev)) = visibility {
                    return Err(duplicate_directive_error(file, *span, prev, "FbVisibility"));
                }
                visibility = Some((*is_public, *span));
            }
        }
    }

    Ok(ResolvedFunctionSignature {
        params: params.map(|(p, _)| p).unwrap_or_default(),
        ret_ty: ret_ty.map(|(ty, _)| ty),
        is_public: visibility.map(|(v, _)| v).unwrap_or(false),
    })
}

fn duplicate_directive_error(file: &str, span: Span, _prev: Span, name: &str) -> SprsError {
    semantic(
        file,
        span,
        20,
        format!("duplicate FunctionBuild directive @{name}"),
        Some("each @FbArgs / @FbRetTy / @FbVisibility may appear at most once".to_string()),
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
                    "multiple `#define FunctionBuild` directives in one file".to_string(),
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

pub fn resolve_function_build_types(
    items: &mut [Item],
    known_structs: &HashSet<String>,
    path: &str,
) -> Result<(), SprsError> {
    for item in items.iter_mut() {
        let Item::FunctionBuildItem(fb) = item else {
            continue;
        };
        for directive in &mut fb.directives {
            match directive {
                FunctionBuildDirective::Args { params, .. } => {
                    for param in params {
                        if let Some(annot) = &mut param.ty {
                            type_helper::resolve_type(&mut annot.ty, known_structs, None)
                                .map_err(|message| semantic(path, param.span, 11, message, None))?;
                        }
                    }
                }
                FunctionBuildDirective::RetTy { ty, span } => {
                    type_helper::resolve_type(ty, known_structs, None)
                        .map_err(|message| semantic(path, *span, 11, message, None))?;
                }
                FunctionBuildDirective::Visibility { .. } => {}
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

/// Load a FunctionBuild declaration source (not a runtime module).
///
/// Follows nested `#define FunctionBuild` only for cycle detection.
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
    resolve_function_build_types(&mut items, &known, &path)?;
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
    use crate::llvm::parser::parse_only;

    fn parse(src: &str) -> Vec<Item> {
        parse_only(src, "test.sprs").expect("parse")
    }

    #[test]
    fn resolves_basic_function_build_signature() {
        let items = parse(
            r#"
function_build AddBuild {
    @FbArgs(lhs >> i64, rhs >> i64);
    @FbRetTy(i64);
    @FbVisibility(pub);
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
    }

    #[test]
    fn rejects_duplicate_directives() {
        let items = parse(
            r#"
function_build Bad {
    @FbRetTy(i64);
    @FbRetTy(str);
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
                assert!(message.contains("@FbRetTy"));
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
    @FbArgs(lhs >> i64, rhs >> i64);
    @FbRetTy(i64);
    @FbVisibility(pub);
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
        let items = parse("#define FunctionBuild a\n#define FunctionBuild b\n");
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
    @FbArgs(value >> i64);
    @FbRetTy(i64);
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
        std::fs::write(dir.join("a.sprs"), "#define FunctionBuild b\n").unwrap();
        std::fs::write(dir.join("b.sprs"), "#define FunctionBuild a\n").unwrap();
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
    @FbArgs(lhs >> i64, rhs >> i64);
    @FbRetTy(i64);
}
function_build InternalBuild {
    @FbRetTy(str);
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
        let mut items = parse("function_build Foo {}\nfn Foo use Foo {}\n");
        let builds = collect_local_function_builds(&items, "test.sprs", false).unwrap();
        let mut registry = FunctionBuildRegistry::default();
        insert_builds(&mut registry, builds).unwrap();
        lower_functions_with_builds(&mut items, &registry, "test.sprs").unwrap();
        assert!(matches!(&items[1], Item::FunctionItem(func) if func.ident == "Foo"));
    }

    #[test]
    fn resolves_named_struct_types_in_function_build() {
        let mut items = parse(
            r#"
struct Job { id >> i64 }
function_build JobFn {
    @FbArgs(job >> Job);
    @FbRetTy(Job);
}
"#,
        );
        let known = known_structs_from_items(&items);
        resolve_function_build_types(&mut items, &known, "test.sprs").unwrap();
        let Item::FunctionBuildItem(fb) = &items[1] else {
            panic!("expected function_build");
        };
        match &fb.directives[0] {
            FunctionBuildDirective::Args { params, .. } => {
                assert_eq!(
                    params[0].ty.as_ref().map(|annot| &annot.ty),
                    Some(&Type::Struct("Job".into()))
                );
            }
            other => panic!("{other:?}"),
        }
        match &fb.directives[1] {
            FunctionBuildDirective::RetTy { ty, .. } => {
                assert_eq!(ty, &Type::Struct("Job".into()));
            }
            other => panic!("{other:?}"),
        }
    }
}

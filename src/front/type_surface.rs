use crate::front::error::ErrorCategory;
use crate::front::lexer::Token;
use crate::front::parse_error::ParserUserError;
use crate::front::span::Span;
use crate::front::type_helper::Type;
use lalrpop_util::ParseError;

pub fn user(
    error: ParserUserError,
) -> ParseError<usize, Token, ParserUserError> {
    ParseError::User { error }
}

pub fn syn(
    number: u32,
    span: Span,
    message: impl Into<String>,
    help: Option<String>,
) -> ParseError<usize, Token, ParserUserError> {
    user(ParserUserError::syntax(number, span, message, help))
}

pub fn sem(
    number: u32,
    span: Span,
    message: impl Into<String>,
    help: Option<String>,
) -> ParseError<usize, Token, ParserUserError> {
    user(ParserUserError::semantic(number, span, message, help))
}

fn legacy_type_help(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "int" => Some(("i64", "use i64 instead of int")),
        "fp" | "fp64" => Some(("f64", "use f64 instead of the old float alias")),
        "fp16" => Some(("f16", "use f16 instead of fp16")),
        "fp32" => Some(("f32", "use f32 instead of fp32")),
        "list" => Some(("List(T)", "use List(T) or List(Any) instead of list")),
        "range" => Some(("Range", "use Range instead of range")),
        "buffer" => Some(("Buffer", "use Buffer instead of buffer")),
        "rawptr" => Some(("RawPtr", "use RawPtr instead of rawptr")),
        "err" => Some((
            "Label(:error, Any)",
            "use Label(:error, Any) instead of err",
        )),
        "atom" | "Atom" => Some(("Label", "Atom is not a surface type; use Label, :name, or Label(:name, T)")),
        "label" => Some(("Label", "use Label instead of label")),
        _ => None,
    }
}

pub fn named_type(
    name: String,
    args: Vec<Type>,
    span: Span,
) -> Result<Type, ParseError<usize, Token, ParserUserError>> {
    if let Some((replacement, help)) = legacy_type_help(&name) {
        return Err(sem(
            11,
            span,
            format!("unknown type `{name}`; use {replacement}"),
            Some(help.to_string()),
        ));
    }
    match (name.as_str(), args.as_slice()) {
        ("Any", []) => Ok(Type::Any),
        ("Any", _) => Err(sem(
            11,
            span,
            "Any does not take type arguments",
            Some("write Any, not Any(...)".to_string()),
        )),
        ("Range", []) => Ok(Type::Range),
        ("Range", _) => Err(sem(
            11,
            span,
            "Range does not take type arguments",
            Some("write Range".to_string()),
        )),
        ("Buffer", []) => Ok(Type::Buffer),
        ("Buffer", _) => Err(sem(
            11,
            span,
            "Buffer does not take type arguments",
            Some("write Buffer".to_string()),
        )),
        ("RawPtr", []) => Ok(Type::RawPtr),
        ("RawPtr", _) => Err(sem(
            11,
            span,
            "RawPtr does not take type arguments",
            Some("write RawPtr".to_string()),
        )),
        ("Label", []) => Ok(Type::Label),
        ("Label", [Type::Atom(_), _]) => Ok(Type::App("Label".into(), args)),
        ("Label", _) => Err(sem(
            11,
            span,
            "Label application must be Label or Label(:name, T)",
            Some("payloadless exact labels use :name; payload labels use Label(:name, T)".to_string()),
        )),
        ("List", [_]) => Ok(Type::App("List".into(), args)),
        ("List", _) => Err(sem(
            11,
            span,
            "List requires exactly one type argument",
            Some("use List(T) or List(Any)".to_string()),
        )),
        ("Process", [_]) => Ok(Type::App("Process".into(), args)),
        ("Process", _) => Err(sem(
            11,
            span,
            "Process requires exactly one type argument",
            Some("use Process(T)".to_string()),
        )),
        ("Self", []) => Ok(Type::SelfType),
        ("Self", _) => Err(sem(
            11,
            span,
            "Self does not take type arguments",
            Some("write Self".to_string()),
        )),
        (_, _) => {
            if !crate::front::ident::is_pascal_case(&name) {
                return Err(sem(
                    25,
                    span,
                    "type names must use PascalCase",
                    Some("use PascalCase (e.g. DmaController, not DMAController)".to_string()),
                ));
            }
            if args.is_empty() {
                Ok(Type::Named(name))
            } else {
                Err(sem(
                    11,
                    span,
                    format!(
                        "unknown type constructor `{name}`; builtin constructors are List(T), Process(T), Label(:name, T)"
                    ),
                    Some("user-defined generic types are not supported".to_string()),
                ))
            }
        }
    }
}

pub fn finish_type(
    id: crate::front::ident::ParsedIdentifier,
    args: Vec<Type>,
) -> Result<Type, ParseError<usize, Token, ParserUserError>> {
    let ty = named_type(id.name.clone(), args, id.span)?;
    if id.escaped && !crate::front::ident::is_keyword(&id.name) {
        return Err(syn(
            8,
            id.span,
            format!("unnecessary identifier escape `^{}`", id.name),
            Some(format!("use {} instead of ^{}", id.name, id.name)),
        ));
    }
    Ok(ty)
}

pub fn reject_label_keyword(span: Span) -> ParseError<usize, Token, ParserUserError> {
    sem(
        11,
        span,
        "unknown type `label`; use Label",
        Some("use Label instead of label".to_string()),
    )
}

#[allow(dead_code)]
fn _category_marker(_: ErrorCategory) {}

use crate::front::error::ErrorCategory;
use crate::front::parse_error::ParserUserError;
use crate::front::span::Span;
use lalrpop_util::ParseError;

use crate::front::lexer::Token;

/// Identifier as recognized by the lexer, before category naming checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIdentifier {
    pub name: String,
    pub escaped: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentCategory {
    /// Functions, including call-site names.
    Function,
    /// Package, import, and FunctionBuild source names.
    Module,
    /// File-scope `var` bindings.
    GlobalVar,
    /// Local variables, parameters, and pattern binders. Allows `_name`.
    Local,
    /// Struct fields and `init` field names.
    Field,
    /// Attach slots (`<:name`).
    AttachSlot,
    /// Macro names (`@name`).
    Macro,
    /// Structs, closed label sets, FunctionBuild, named types, type parameters.
    PascalType,
    /// Open labels (`:name`) and `label :name` declarations.
    OpenLabel,
    /// Closed label set members.
    ClosedMember,
}

impl IdentCategory {
    fn message(self) -> &'static str {
        match self {
            IdentCategory::Function => "function names must use snake_case",
            IdentCategory::Module => "module names must use snake_case",
            IdentCategory::GlobalVar => "global variable names must use snake_case",
            IdentCategory::Local => "variable names must use snake_case",
            IdentCategory::Field => "field names must use snake_case",
            IdentCategory::AttachSlot => "attach slot names must use snake_case",
            IdentCategory::Macro => "macro names must use snake_case",
            IdentCategory::PascalType => "type names must use PascalCase",
            IdentCategory::OpenLabel => "label names must use snake_case",
            IdentCategory::ClosedMember => "label member names must use snake_case",
        }
    }

    fn help(self) -> &'static str {
        match self {
            IdentCategory::PascalType => "use PascalCase (e.g. DmaController, not DMAController)",
            _ => "use snake_case (e.g. start_dma, not StartDMA)",
        }
    }

    fn allows_leading_underscore(self) -> bool {
        matches!(self, IdentCategory::Local)
    }

    fn is_pascal(self) -> bool {
        matches!(self, IdentCategory::PascalType)
    }
}

pub fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "else"
            | "while"
            | "fn"
            | "use"
            | "function_build"
            | "private"
            | "return"
            | "pkg"
            | "import"
            | "var"
            | "pub"
            | "struct"
            | "ambi"
            | "new"
            | "destroy"
            | "exist"
            | "unsafe"
            | "defer"
            | "match"
            | "case"
            | "break"
            | "true"
            | "false"
            | "bool"
            | "str"
            | "unit"
            | "i8"
            | "u8"
            | "i16"
            | "u16"
            | "i32"
            | "u32"
            | "i64"
            | "u64"
            | "f16"
            | "f32"
            | "f64"
            | "label"
            | "init"
            | "source"
            | "params"
            | "return_type"
            | "visibility"
            | "type_param"
            | "when"
            | "is"
            | "neq"
            | "and"
            | "or"
            | "not"
    )
}

pub fn is_snake_case(name: &str) -> bool {
    if name.is_empty() || name.contains("__") || name.ends_with('_') {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

pub fn is_pascal_case(name: &str) -> bool {
    if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return false;
    }
    let chars: Vec<char> = name.chars().collect();
    if !chars[0].is_ascii_uppercase() {
        return false;
    }
    chars
        .windows(2)
        .all(|pair| !(pair[0].is_ascii_uppercase() && pair[1].is_ascii_uppercase()))
}

fn naming_ok(name: &str, category: IdentCategory) -> bool {
    if name.starts_with("__") {
        return false;
    }
    if name == "_" {
        return false;
    }
    if let Some(rest) = name.strip_prefix('_') {
        return category.allows_leading_underscore() && is_snake_case(rest);
    }
    if category.is_pascal() {
        is_pascal_case(name)
    } else {
        is_snake_case(name)
    }
}

pub fn finish(
    id: ParsedIdentifier,
    category: IdentCategory,
) -> Result<String, ParseError<usize, Token, ParserUserError>> {
    if !naming_ok(&id.name, category) {
        return Err(ParseError::User {
            error: ParserUserError::new(
                ErrorCategory::Semantic,
                25,
                id.span,
                category.message(),
                Some(category.help().to_string()),
            ),
        });
    }
    if id.escaped && !is_keyword(&id.name) {
        return Err(ParseError::User {
            error: ParserUserError::syntax(
                8,
                id.span,
                format!("unnecessary identifier escape `^{}`", id.name),
                Some(format!("use {} instead of ^{}", id.name, id.name)),
            ),
        });
    }
    Ok(id.name)
}

pub fn finish_macro(
    name: String,
    span: Span,
) -> Result<String, ParseError<usize, Token, ParserUserError>> {
    finish(
        ParsedIdentifier {
            name,
            escaped: false,
            span,
        },
        IdentCategory::Macro,
    )
}

pub fn validate_static_label(
    raw: &str,
    span: Span,
) -> Result<String, ParseError<usize, Token, ParserUserError>> {
    if let Some((set, member)) = raw.split_once('.') {
        let set_id = ParsedIdentifier {
            name: set.to_string(),
            escaped: false,
            span,
        };
        let member_id = ParsedIdentifier {
            name: member.to_string(),
            escaped: false,
            span,
        };
        finish(set_id, IdentCategory::PascalType)?;
        finish(member_id, IdentCategory::ClosedMember)?;
        Ok(raw.to_string())
    } else {
        finish(
            ParsedIdentifier {
                name: raw.to_string(),
                escaped: false,
                span,
            },
            IdentCategory::OpenLabel,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_and_pascal_examples() {
        assert!(is_snake_case("start_dma"));
        assert!(is_snake_case("i2c_bus"));
        assert!(is_snake_case("uart0"));
        assert!(is_pascal_case("T"));
        assert!(is_pascal_case("DmaController"));
        assert!(is_pascal_case("Dma2Controller"));
        assert!(!is_pascal_case("DMAController"));
        assert!(!is_snake_case("StartDMA"));
        assert!(!is_snake_case("_hidden"));
    }

    #[test]
    fn local_underscore_allowed() {
        let span = Span::new(0, 5);
        let id = ParsedIdentifier {
            name: "_tmp".into(),
            escaped: false,
            span,
        };
        assert!(finish(id.clone(), IdentCategory::Local).is_ok());
        assert!(finish(id, IdentCategory::Function).is_err());
    }

    #[test]
    fn unnecessary_escape_after_naming() {
        let span = Span::new(0, 4);
        let bad = ParsedIdentifier {
            name: "BadName".into(),
            escaped: true,
            span,
        };
        let err = finish(bad, IdentCategory::Local).unwrap_err();
        match err {
            ParseError::User { error } => {
                assert_eq!(error.category, ErrorCategory::Semantic);
                assert_eq!(error.number, 25);
            }
            other => panic!("unexpected {other:?}"),
        }
        let escaped_ok = ParsedIdentifier {
            name: "foo".into(),
            escaped: true,
            span,
        };
        let err = finish(escaped_ok, IdentCategory::Local).unwrap_err();
        match err {
            ParseError::User { error } => {
                assert_eq!(error.number, 8);
                assert_eq!(error.category, ErrorCategory::Syntax);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

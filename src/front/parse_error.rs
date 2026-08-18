use crate::front::error::ErrorCategory;
use crate::front::span::Span;

/// Structured user error produced by the lexer or LALRPOP actions.
///
/// Replaces string-only `ParseError::User` so `error_reporter` can keep the
/// original span, code, and help text.
#[derive(Debug, Clone, PartialEq)]
pub struct ParserUserError {
    pub category: ErrorCategory,
    pub number: u32,
    pub span: Span,
    pub message: String,
    pub help: Option<String>,
}

impl ParserUserError {
    pub fn new(
        category: ErrorCategory,
        number: u32,
        span: Span,
        message: impl Into<String>,
        help: Option<String>,
    ) -> Self {
        Self {
            category,
            number,
            span,
            message: message.into(),
            help,
        }
    }

    pub fn syntax(
        number: u32,
        span: Span,
        message: impl Into<String>,
        help: Option<String>,
    ) -> Self {
        Self::new(ErrorCategory::Syntax, number, span, message, help)
    }

    pub fn semantic(
        number: u32,
        span: Span,
        message: impl Into<String>,
        help: Option<String>,
    ) -> Self {
        Self::new(ErrorCategory::Semantic, number, span, message, help)
    }

    pub fn invalid_token(span: Span, message: impl Into<String>) -> Self {
        Self::syntax(1, span, message, None)
    }
}

impl std::fmt::Display for ParserUserError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

//! Structured error reporting: type definitions and output rendering.

use crate::front::span::Span;

/// Stable error code that does not change across spec revisions.
/// Format: SPRS-<CAT>-<NNN>
#[derive(Debug, Clone)]
pub struct ErrorCode {
    pub category: ErrorCategory,
    pub number: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Syntax error: lalrpop failed to parse.
    Syntax,
    /// Type error: e.g. return-type annotation does not match actual return value.
    Type,
    /// Semantic error: undefined variable, unknown macro, etc.
    Semantic,
}

impl ErrorCode {
    /// Returns the string representation, e.g. "SPRS-SYN-001".
    pub fn as_string(&self) -> String {
        let error_cat = match self.category {
            ErrorCategory::Syntax => "SYN",
            ErrorCategory::Type => "TYP",
            ErrorCategory::Semantic => "SEM",
        };
        format!("SPRS-{}-{:03}", error_cat, self.number)
    }
}

/// Source file and span where an error occurred.
#[derive(Debug, Clone)]
pub struct Location {
    pub file: String,
    pub span: Span,
}

impl Location {
    pub fn new(file: String, span: Span) -> Self {
        Self { file, span }
    }
}

/// Structured error emitted by the compiler.
#[derive(Debug, Clone)]
pub enum SprsError {
    /// Structured form of lalrpop's ParseError.
    Parse {
        code: ErrorCode,
        location: Location,
        message: String,
        expected: Vec<String>,
        help: Option<String>,
    },
    /// Semantic error (undefined variable, unknown macro, etc.)
    Semantic {
        code: ErrorCode,
        location: Location,
        message: String,
        help: Option<String>,
    },
    /// Type error (return-type mismatch, etc.)
    Type {
        code: ErrorCode,
        location: Location,
        message: String,
        expected_type: Option<String>,
        actual_type: Option<String>,
        help: Option<String>,
    },
    /// Compiler-internal error (bug).
    Internal {
        message: String,
        location: Option<Location>,
    },
}

impl std::fmt::Display for SprsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SprsError::Parse { code, message, .. } => {
                write!(formatter, "{}: {}", code.as_string(), message)
            }
            SprsError::Semantic { code, message, .. } => {
                write!(formatter, "{}: {}", code.as_string(), message)
            }
            SprsError::Type { code, message, .. } => {
                write!(formatter, "{}: {}", code.as_string(), message)
            }
            SprsError::Internal { message, .. } => {
                write!(formatter, "Internal error: {}", message)
            }
        }
    }
}

impl std::error::Error for SprsError {}

/// Legacy conversion from String-based errors.
/// Can be removed once all call sites use SprsError directly.
impl From<String> for SprsError {
    fn from(msg: String) -> Self {
        SprsError::Internal {
            message: msg,
            location: None,
        }
    }
}

/// Output format for error rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorFormat {
    Human,
    Json,
}

impl ErrorFormat {
    pub fn from_str(format_str: &str) -> Result<Self, String> {
        match format_str {
            "human" => Ok(ErrorFormat::Human),
            "json" => Ok(ErrorFormat::Json),
            _ => Err(format!(
                "Unknown error format: {} (use 'human' or 'json')",
                format_str
            )),
        }
    }
}

/// Convert a byte offset to 1-based (line, column) in the source text.
fn get_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i == offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Return the source line at the given 1-based line number.
fn get_snippet(source: &str, line_number: usize) -> String {
    source
        .lines()
        .nth(line_number.saturating_sub(1))
        .unwrap_or("")
        .to_string()
}

/// Render a SprsError as a string in the requested format.
pub fn render(error: &SprsError, format: ErrorFormat, source: &str) -> String {
    match format {
        ErrorFormat::Json => render_json(error, source),
        ErrorFormat::Human => render_human(error, source),
    }
}

fn render_json(error: &SprsError, source: &str) -> String {
    match error {
        SprsError::Parse {
            code,
            location,
            message,
            expected,
            help,
        } => {
            let (line, col) = get_line_col(source, location.span.start);
            let (end_line, end_col) = get_line_col(source, location.span.end);
            let snippet = get_snippet(source, line);
            let expected_json = expected
                .iter()
                .map(|expected_token| format!("\"{}\"", expected_token.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(",");
            let help_json = match help {
                Some(help_text) => format!("\"{}\"", help_text.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            format!(
                r#"{{"code":"{}","category":"Syntax","phase":"compile","severity":"error","message":"{}","location":{{"file":"{}","line":{},"column":{},"end_line":{},"end_column":{},"snippet":"{}"}},"expected":[{}],"help":{}}}"#,
                code.as_string(),
                message.replace('"', "\\\""),
                location.file.replace('"', "\\\""),
                line,
                col,
                end_line,
                end_col,
                snippet.replace('"', "\\\"").replace('\n', "\\n"),
                expected_json,
                help_json
            )
        }
        SprsError::Semantic {
            code,
            location,
            message,
            help,
        } => {
            let (line, col) = get_line_col(source, location.span.start);
            let (end_line, end_col) = get_line_col(source, location.span.end);
            let snippet = get_snippet(source, line);
            let help_json = match help {
                Some(help_text) => format!("\"{}\"", help_text.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            format!(
                r#"{{"code":"{}","category":"Semantic","phase":"compile","severity":"error","message":"{}","location":{{"file":"{}","line":{},"column":{},"end_line":{},"end_column":{},"snippet":"{}"}},"help":{}}}"#,
                code.as_string(),
                message.replace('"', "\\\""),
                location.file.replace('"', "\\\""),
                line,
                col,
                end_line,
                end_col,
                snippet.replace('"', "\\\"").replace('\n', "\\n"),
                help_json
            )
        }
        SprsError::Type {
            code,
            location,
            message,
            expected_type,
            actual_type,
            help,
        } => {
            let (line, col) = get_line_col(source, location.span.start);
            let (end_line, end_col) = get_line_col(source, location.span.end);
            let snippet = get_snippet(source, line);
            let expected_type_json = match expected_type {
                Some(type_name) => format!("\"{}\"", type_name.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            let actual_type_json = match actual_type {
                Some(type_name) => format!("\"{}\"", type_name.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            let help_json = match help {
                Some(help_text) => format!("\"{}\"", help_text.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            format!(
                r#"{{"code":"{}","category":"Type","phase":"compile","severity":"error","message":"{}","location":{{"file":"{}","line":{},"column":{},"end_line":{},"end_column":{},"snippet":"{}"}},"expected_type":{},"actual_type":{},"help":{}}}"#,
                code.as_string(),
                message.replace('"', "\\\""),
                location.file.replace('"', "\\\""),
                line,
                col,
                end_line,
                end_col,
                snippet.replace('"', "\\\"").replace('\n', "\\n"),
                expected_type_json,
                actual_type_json,
                help_json
            )
        }
        SprsError::Internal { message, location } => {
            let (line, col, file) = match location {
                Some(loc) => {
                    let (line_num, col_num) = get_line_col(source, loc.span.start);
                    (line_num, col_num, loc.file.clone())
                }
                None => (0, 0, "<unknown>".to_string()),
            };
            format!(
                r#"{{"code":"SPRS-INTERNAL","category":"Internal","phase":"compile","severity":"error","message":"{}","location":{{"file":"{}","line":{},"column":{}}}}}"#,
                message.replace('"', "\\\""),
                file.replace('"', "\\\""),
                line,
                col
            )
        }
    }
}

fn render_human(error: &SprsError, source: &str) -> String {
    match error {
        SprsError::Parse {
            code,
            location,
            message,
            expected,
            help,
        } => {
            let (line, col) = get_line_col(source, location.span.start);
            let snippet = get_snippet(source, line);
            let pointer = " ".repeat(col) + "^";
            let mut output = format!(
                "error[{}]: {}\n  --> {}:{}:{}\n   |\n{: >3} | {}\n   | {}\n",
                code.as_string(),
                message,
                location.file,
                line,
                col,
                line,
                snippet,
                pointer
            );
            if !expected.is_empty() {
                output.push_str(&format!("   |\n   = expected: {}\n", expected.join(", ")));
            }
            if let Some(help_text) = help {
                output.push_str(&format!("help: {}\n", help_text));
            }
            output
        }
        SprsError::Semantic {
            code,
            location,
            message,
            help,
        } => {
            let (line, col) = get_line_col(source, location.span.start);
            let snippet = get_snippet(source, line);
            let pointer = " ".repeat(col) + "^";
            let mut output = format!(
                "error[{}]: {}\n  --> {}:{}:{}\n   |\n{: >3} | {}\n   | {}\n",
                code.as_string(),
                message,
                location.file,
                line,
                col,
                line,
                snippet,
                pointer
            );
            if let Some(help_text) = help {
                output.push_str(&format!("help: {}\n", help_text));
            }
            output
        }
        SprsError::Type {
            code,
            location,
            message,
            expected_type,
            actual_type,
            help,
        } => {
            let (line, col) = get_line_col(source, location.span.start);
            let snippet = get_snippet(source, line);
            let pointer = " ".repeat(col) + "^";
            let mut output = format!(
                "error[{}]: {}\n  --> {}:{}:{}\n   |\n{: >3} | {}\n   | {}\n",
                code.as_string(),
                message,
                location.file,
                line,
                col,
                line,
                snippet,
                pointer
            );
            if let (Some(expected_type_str), Some(actual_type_str)) = (expected_type, actual_type) {
                output.push_str(&format!(
                    "   |\n   = expected: {}, found: {}\n",
                    expected_type_str, actual_type_str
                ));
            }
            if let Some(help_text) = help {
                output.push_str(&format!("help: {}\n", help_text));
            }
            output
        }
        SprsError::Internal { message, location } => match location {
            Some(loc) => {
                let (line, col) = get_line_col(source, loc.span.start);
                format!(
                    "internal error: {}\n  --> {}:{}:{}\n",
                    message, loc.file, line, col
                )
            }
            None => format!("internal error: {}\n", message),
        },
    }
}

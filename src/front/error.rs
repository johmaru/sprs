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
    JsonPretty,
}

impl ErrorFormat {
    pub fn from_str(format_str: &str) -> Result<Self, String> {
        match format_str {
            "human" => Ok(ErrorFormat::Human),
            "json" => Ok(ErrorFormat::Json),
            "json-pretty" => Ok(ErrorFormat::JsonPretty),
            _ => Err(format!(
                "Unknown error format: {} (use 'human', 'json', or 'json-pretty')",
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
        ErrorFormat::Json => render_json(error, source, false),
        ErrorFormat::JsonPretty => render_json(error, source, true),
        ErrorFormat::Human => render_human(error, source),
    }
}

fn render_json(error: &SprsError, source: &str, pretty: bool) -> String {
    let report = build_json_report(error, source);
    if pretty {
        serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
            format!("{{\"error\":\"Failed to serialize error report: {}\"}}", e)
        })
    } else {
        serde_json::to_string(&report).unwrap_or_else(|e| {
            format!("{{\"error\":\"Failed to serialize error report: {}\"}}", e)
        })
    }
}

/// Presentation DTO for JSON serialization.
/// Flattens SprsError's internal representation into the stable schema
/// consumed by AI agents and tools.
#[derive(serde::Serialize)]
struct JsonErrorReport {
    code: String,
    category: String,
    phase: String,
    severity: String,
    message: String,
    location: JsonLocation,
    expected: Vec<String>,
    expected_type: Option<String>,
    actual_type: Option<String>,
    help: Option<String>,
}

#[derive(serde::Serialize)]
struct JsonLocation {
    file: String,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
    snippet: String,
}

/// Build a JsonErrorReport from a SprsError and its source text.
/// This centralizes all formatting logic (code string, line/col resolution,
/// snippet extraction) that was previously duplicated in hand-written JSON.
fn build_json_report(error: &SprsError, source: &str) -> JsonErrorReport {
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
            JsonErrorReport {
                code: code.as_string(),
                category: "Syntax".to_string(),
                phase: "compile".to_string(),
                severity: "error".to_string(),
                message: message.clone(),
                location: JsonLocation {
                    file: location.file.clone(),
                    line,
                    column: col,
                    end_line,
                    end_column: end_col,
                    snippet: get_snippet(source, line),
                },
                expected: expected.clone(),
                expected_type: None,
                actual_type: None,
                help: help.clone(),
            }
        }
        SprsError::Semantic {
            code,
            location,
            message,
            help,
        } => {
            let (line, col) = get_line_col(source, location.span.start);
            let (end_line, end_col) = get_line_col(source, location.span.end);
            JsonErrorReport {
                code: code.as_string(),
                category: "Semantic".to_string(),
                phase: "compile".to_string(),
                severity: "error".to_string(),
                message: message.clone(),
                location: JsonLocation {
                    file: location.file.clone(),
                    line,
                    column: col,
                    end_line,
                    end_column: end_col,
                    snippet: get_snippet(source, line),
                },
                expected: vec![],
                expected_type: None,
                actual_type: None,
                help: help.clone(),
            }
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
            JsonErrorReport {
                code: code.as_string(),
                category: "Type".to_string(),
                phase: "compile".to_string(),
                severity: "error".to_string(),
                message: message.clone(),
                location: JsonLocation {
                    file: location.file.clone(),
                    line,
                    column: col,
                    end_line,
                    end_column: end_col,
                    snippet: get_snippet(source, line),
                },
                expected: vec![],
                expected_type: expected_type.clone(),
                actual_type: actual_type.clone(),
                help: help.clone(),
            }
        }
        SprsError::Internal { message, location } => {
            let (file, line, col, end_line, end_col) = match location {
                Some(loc) => {
                    let (line_num, col_num) = get_line_col(source, loc.span.start);
                    let (end_line_num, end_col_num) = get_line_col(source, loc.span.end);
                    (
                        loc.file.clone(),
                        line_num,
                        col_num,
                        end_line_num,
                        end_col_num,
                    )
                }
                None => ("<unknown>".to_string(), 0, 0, 0, 0),
            };
            JsonErrorReport {
                code: "SPRS-INTERNAL".to_string(),
                category: "Internal".to_string(),
                phase: "compile".to_string(),
                severity: "error".to_string(),
                message: message.clone(),
                location: JsonLocation {
                    file,
                    line,
                    column: col,
                    end_line,
                    end_column: end_col,
                    snippet: if line == 0 {
                        String::new()
                    } else {
                        get_snippet(source, line)
                    },
                },
                expected: vec![],
                expected_type: None,
                actual_type: None,
                help: None,
            }
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

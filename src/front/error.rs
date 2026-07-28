//! 構造化エラーレポートの型定義と出力。

use crate::front::span::Span;

/// 安定エラーコード。仕様変更で変わらない ID。
/// 形式: SPRS-<CAT>-<NNN>
#[derive(Debug, Clone)]
pub struct ErrorCode {
    pub category: ErrorCategory,
    pub number: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// 構文エラー: lalrpop がパース失敗
    Syntax,
    /// 型エラー: 関数の戻り値型注釈と実際の戻り値の不一致等
    Type,
    /// 意味エラー: 未定義変数、未定義関数、未知のマクロ等
    Semantic,
}

impl ErrorCode {
    /// SPRS-SYN-001 形式の文字列表現
    pub fn as_string(&self) -> String {
        let cat = match self.category {
            ErrorCategory::Syntax => "SYN",
            ErrorCategory::Type => "TYP",
            ErrorCategory::Semantic => "SEM",
        };
        format!("SPRS-{}-{:03}", cat, self.number)
    }
}

/// エラーの発生元ファイルと位置
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

/// 構造化エラー
#[derive(Debug, Clone)]
pub enum SprsError {
    /// lalrpop の ParseError を構造化したもの
    Parse {
        code: ErrorCode,
        location: Location,
        message: String,
        expected: Vec<String>,
        help: Option<String>,
    },
    /// 意味エラー（未定義変数、未知のマクロ等）
    Semantic {
        code: ErrorCode,
        location: Location,
        message: String,
        help: Option<String>,
    },
    /// 型エラー（戻り値型不一致等）
    Type {
        code: ErrorCode,
        location: Location,
        message: String,
        expected_type: Option<String>,
        actual_type: Option<String>,
        help: Option<String>,
    },
    /// コンパイラ内部エラー（バグ）
    Internal {
        message: String,
        location: Option<Location>,
    },
}

impl std::fmt::Display for SprsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SprsError::Parse { code, message, .. } => write!(f, "{}: {}", code.as_string(), message),
            SprsError::Semantic { code, message, .. } => write!(f, "{}: {}", code.as_string(), message),
            SprsError::Type { code, message, .. } => write!(f, "{}: {}", code.as_string(), message),
            SprsError::Internal { message, .. } => write!(f, "Internal error: {}", message),
        }
    }
}

impl std::error::Error for SprsError {}

/// 既存の String ベースエラーからの移行用。
/// Phase 2 で全サイトが SprsError に置き換わったら削除可能。
impl From<String> for SprsError {
    fn from(msg: String) -> Self {
        SprsError::Internal {
            message: msg,
            location: None,
        }
    }
}

/// エラー出力フォーマット
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorFormat {
    Human,
    Json,
}

impl ErrorFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "human" => Ok(ErrorFormat::Human),
            "json" => Ok(ErrorFormat::Json),
            _ => Err(format!(
                "Unknown error format: {} (use 'human' or 'json')",
                s
            )),
        }
    }
}

/// バイトオフセットから行番号と列番号を計算
fn get_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in source.char_indices() {
        if i == offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// 指定行のソースコードスニペットを取得
fn get_snippet(source: &str, line_number: usize) -> String {
    source
        .lines()
        .nth(line_number.saturating_sub(1))
        .unwrap_or("")
        .to_string()
}

/// SprsError を文字列として出力
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
                .map(|e| format!("\"{}\"", e.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(",");
            let help_json = match help {
                Some(h) => format!("\"{}\"", h.replace('"', "\\\"")),
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
                Some(h) => format!("\"{}\"", h.replace('"', "\\\"")),
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
            let et = match expected_type {
                Some(t) => format!("\"{}\"", t.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            let at = match actual_type {
                Some(t) => format!("\"{}\"", t.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            let help_json = match help {
                Some(h) => format!("\"{}\"", h.replace('"', "\\\"")),
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
                et,
                at,
                help_json
            )
        }
        SprsError::Internal { message, location } => {
            let (line, col, file) = match location {
                Some(loc) => {
                    let (l, c) = get_line_col(source, loc.span.start);
                    (l, c, loc.file.clone())
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
            let mut out = format!(
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
                out.push_str(&format!("   |\n   = expected: {}\n", expected.join(", ")));
            }
            if let Some(h) = help {
                out.push_str(&format!("help: {}\n", h));
            }
            out
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
            let mut out = format!(
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
            if let Some(h) = help {
                out.push_str(&format!("help: {}\n", h));
            }
            out
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
            let mut out = format!(
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
            if let (Some(et), Some(at)) = (expected_type, actual_type) {
                out.push_str(&format!("   |\n   = expected: {}, found: {}\n", et, at));
            }
            if let Some(h) = help {
                out.push_str(&format!("help: {}\n", h));
            }
            out
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

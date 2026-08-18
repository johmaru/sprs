# 構造化エラーレポート Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** sprs コンパイラのエラーを構造化データ（SprsError enum）として出力し、AI agent がエラーを機械的に解釈して修正案を出せるようにする。

**Architecture:** Phase 1 で parse error を構造化（基盤工事不要）。Phase 2 で AST に Span を thread し（Spanned<T> ラッパー + lalrpop @L/@R マーカー）、意味/型エラーも同じスキーマで出力。出力は human（デフォルト）と json（`--error-format=json`）の2モード。

**Tech Stack:** Rust, lalrpop 0.23.1, inkwell (LLVM 22.1.8), logos

## Global Constraints

- 言語名は `env!("CARGO_PKG_NAME")` + `naming.rs` から取得（ハードコード禁止）
- `Box::leak` 禁止（rs-box-leak ルール）
- `grammar.rs` は lalrpop が `grammar.lalrpop` からビルド時に再生成。`git checkout src/grammar.rs` してはいけない。変更後は `touch src/grammar.lalrpop` で再生成を強制
- テスト結果は PASS/XFAIL/FAIL の3つ全てを明示的に報告
- 応答は日本語（force-japanese ルール）
- lalrpop 0.23.1 で AST ノードに span を付与する場合、`<>` は単一マッチ値フォールスルーであり span タプルではない。span 取得には各規則に `@L`/`@R` マーカーを明示追加する必要がある。`@L` は右隣トークンの開始バイト位置、`@R` は左隣トークンの最終バイト+1、両方 `usize` に bind
- sprs の AST（ast.rs）は現状 span を一切持たない。grammar.lalrpop は `@L`/`@R` を1箇所も使っていない

**Spec:** `docs/superpowers/specs/2026-07-27-structured-error-reporting-design.md`

---

## File Structure

### 新規ファイル
- `src/front/error.rs` — `SprsError` enum, `ErrorCode`, `ErrorCategory`, `Location`, `ErrorFormat`, `render()` 関数
- `src/front/span.rs` — `Span` 構造体と `Spanned<T>` ラッパー（ast.rs が大きくなりすぎないよう分離）

### 変更ファイル（Phase 1）
- `src/front/mod.rs` — `error`, `span` モジュール追加
- `src/llvm/parser.rs` — `parse_only` 戻り値を `Result<Vec<Item>, SprsError>` に
- `src/llvm/error_helper.rs` — `format_parse_error` を `ParseError` → `SprsError::Parse` 変換に書き換え
- `src/main.rs` — `--error-format` フラグ追加
- `src/llvm/llvm_executer.rs` — エラー出力を `SprsError` 対応
- `src/llvm/module_loader.rs` — `load_and_compile_module` 戻り値を `Result<_, SprsError>` に

### 変更ファイル（Phase 2）
- `src/front/ast.rs` — 全 enum/struct を `Spanned<...>` で包む
- `src/grammar.lalrpop` — 全規則に `@L`/`@R` マーカー追加、`Spanned::new()` 生成
- `src/llvm/codegen.rs` — `compile_expr`, `infer_type`, `get_known_type_from_expr`, `get_expr_name` のシグネチャ変更。`Result<_, String>` → `Result<_, SprsError>`
- `src/llvm/compiler.rs` — `sources: HashMap<String, String>` フィールド追加（advisory 指摘の source 伝達問題対応）
- `src/llvm/arithmetic.rs`, `data_structures.rs`, `macros.rs`, `value.rs`, `variable.rs`, `comparison.rs`, `control_flow.rs` — 全 `Result<_, String>` → `Result<_, SprsError>`

---

# Phase 1: parse error 構造化（基盤工事不要）

## Task 1: Span と Spanned<T> の最小定義

**Files:**
- Create: `src/front/span.rs`
- Modify: `src/front/mod.rs`

**Interfaces:**
- Produces: `Span { start: usize, end: usize }`, `Spanned<T> { node: T, span: Span }`, `Span::DUMMY`

- [ ] **Step 1: src/front/span.rs を作成**

```rust
//! ソースコード位置情報の表現。

/// ソースコード上の範囲をバイトオフセットで表現。
/// lexer.rs の logos::span と互換。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// 空スパン（位置不明・合成ノード用）
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// AST ノードに span を付与するラッパー。
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}
```

- [ ] **Step 2: src/front/mod.rs にモジュール追加**

`src/front/mod.rs` に `pub mod span;` を追加:

```rust
pub mod ast;
pub mod lexer;
pub mod span;
pub mod type_helper;
```

- [ ] **Step 3: ビルド確認**

Run: `cargo build 2>&1 | tail -5`
Expected: warning は出るがエラーなしでビルド成功

- [ ] **Step 4: Commit**

```bash
git add src/front/span.rs src/front/mod.rs
git commit -m "feat: Span と Spanned<T> ラッパーを追加（Phase 1 基盤）"
```

---

## Task 2: SprsError enum と ErrorCode の定義

**Files:**
- Create: `src/front/error.rs`
- Modify: `src/front/mod.rs`

**Interfaces:**
- Consumes: `Span`, `Spanned` from `src/front/span.rs`
- Produces: `SprsError` enum, `ErrorCode`, `ErrorCategory`, `Location`, `ErrorFormat`, `render()`

- [ ] **Step 1: src/front/error.rs を作成**

```rust
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
            _ => Err(format!("Unknown error format: {} (use 'human' or 'json')", s)),
        }
    }
}

/// バイトオフセットから行番号と列番号を計算（error_helper.rs から移動）
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
        SprsError::Parse { code, location, message, expected, help } => {
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
                line, col, end_line, end_col,
                snippet.replace('"', "\\\"").replace('\n', "\\n"),
                expected_json,
                help_json
            )
        }
        SprsError::Semantic { code, location, message, help } => {
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
                line, col, end_line, end_col,
                snippet.replace('"', "\\\"").replace('\n', "\\n"),
                help_json
            )
        }
        SprsError::Type { code, location, message, expected_type, actual_type, help } => {
            let (line, col) = get_line_col(source, location.span.start);
            let (end_line, end_col) = get_line_col(source, location.span.end);
            let snippet = get_snippet(source, line);
            let et = match expected_type { Some(t) => format!("\"{}\"", t.replace('"', "\\\"")), None => "null".to_string() };
            let at = match actual_type { Some(t) => format!("\"{}\"", t.replace('"', "\\\"")), None => "null".to_string() };
            let help_json = match help {
                Some(h) => format!("\"{}\"", h.replace('"', "\\\"")),
                None => "null".to_string(),
            };
            format!(
                r#"{{"code":"{}","category":"Type","phase":"compile","severity":"error","message":"{}","location":{{"file":"{}","line":{},"column":{},"end_line":{},"end_column":{},"snippet":"{}"}},"expected_type":{},"actual_type":{},"help":{}}}"#,
                code.as_string(),
                message.replace('"', "\\\""),
                location.file.replace('"', "\\\""),
                line, col, end_line, end_col,
                snippet.replace('"', "\\\"").replace('\n', "\\n"),
                et, at, help_json
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
                line, col
            )
        }
    }
}

fn render_human(error: &SprsError, source: &str) -> String {
    match error {
        SprsError::Parse { code, location, message, expected, help } => {
            let (line, col) = get_line_col(source, location.span.start);
            let snippet = get_snippet(source, line);
            let pointer = " ".repeat(col) + "^";
            let mut out = format!(
                "error[{}]: {}\n  --> {}:{}:{}\n   |\n{: >3} | {}\n   | {}\n",
                code.as_string(), message, location.file, line, col, line, snippet, pointer
            );
            if !expected.is_empty() {
                out.push_str(&format!("   |\n   = expected: {}\n", expected.join(", ")));
            }
            if let Some(h) = help {
                out.push_str(&format!("help: {}\n", h));
            }
            out
        }
        SprsError::Semantic { code, location, message, help } => {
            let (line, col) = get_line_col(source, location.span.start);
            let snippet = get_snippet(source, line);
            let pointer = " ".repeat(col) + "^";
            let mut out = format!(
                "error[{}]: {}\n  --> {}:{}:{}\n   |\n{: >3} | {}\n   | {}\n",
                code.as_string(), message, location.file, line, col, line, snippet, pointer
            );
            if let Some(h) = help {
                out.push_str(&format!("help: {}\n", h));
            }
            out
        }
        SprsError::Type { code, location, message, expected_type, actual_type, help } => {
            let (line, col) = get_line_col(source, location.span.start);
            let snippet = get_snippet(source, line);
            let pointer = " ".repeat(col) + "^";
            let mut out = format!(
                "error[{}]: {}\n  --> {}:{}:{}\n   |\n{: >3} | {}\n   | {}\n",
                code.as_string(), message, location.file, line, col, line, snippet, pointer
            );
            if let (Some(et), Some(at)) = (expected_type, actual_type) {
                out.push_str(&format!("   |\n   = expected: {}, found: {}\n", et, at));
            }
            if let Some(h) = help {
                out.push_str(&format!("help: {}\n", h));
            }
            out
        }
        SprsError::Internal { message, location } => {
            match location {
                Some(loc) => {
                    let (line, col) = get_line_col(source, loc.span.start);
                    format!(
                        "internal error: {}\n  --> {}:{}:{}\n",
                        message, loc.file, line, col
                    )
                }
                None => format!("internal error: {}\n", message),
            }
        }
    }
}
```

- [ ] **Step 2: src/front/mod.rs に error モジュール追加**

```rust
pub mod ast;
pub mod error;
pub mod lexer;
pub mod span;
pub mod type_helper;
```

- [ ] **Step 3: ビルド確認**

Run: `cargo build 2>&1 | tail -10`
Expected: エラーなし（warning は許容）

- [ ] **Step 4: Commit**

```bash
git add src/front/error.rs src/front/mod.rs
git commit -m "feat: SprsError enum と render() を追加（Phase 1）"
```

---

## Task 3: parse error の SprsError::Parse 変換

**Files:**
- Modify: `src/llvm/error_helper.rs`
- Modify: `src/llvm/parser.rs`

**Interfaces:**
- Consumes: `SprsError`, `ErrorCode`, `ErrorCategory`, `Location` from `src/front/error.rs`, `Span` from `src/front/span.rs`
- Produces: `format_parse_error()` → `to_sprs_error()` 変換関数、`parse_only` が `Result<Vec<Item>, SprsError>` を返す

- [ ] **Step 1: error_helper.rs を書き換え**

`src/llvm/error_helper.rs` の `format_parse_error` を `to_sprs_error` に置き換え。既存の `get_line_col` / `get_snippet` は `src/front/error.rs` に移動済みなので削除:

```rust
use crate::front::error::{SprsError, ErrorCode, ErrorCategory, Location};
use crate::front::lexer::Token;
use crate::front::span::Span;
use lalrpop_util::ParseError;

/// lalrpop の ParseError を SprsError::Parse に変換
pub fn to_sprs_error(
    source: &str,
    file_path: &str,
    error: ParseError<usize, Token, String>,
) -> SprsError {
    match error {
        ParseError::InvalidToken { location } => SprsError::Parse {
            code: ErrorCode { category: ErrorCategory::Syntax, number: 1 },
            location: Location::new(file_path.to_string(), Span::new(location, location)),
            message: "InvalidToken".to_string(),
            expected: vec![],
            help: None,
        },
        ParseError::UnrecognizedToken { token: (start, token, _end), expected } => {
            let span = Span::new(start, start);
            let expected_strs: Vec<String> = expected.iter().map(|e| format!("{:?}", e)).collect();
            SprsError::Parse {
                code: ErrorCode { category: ErrorCategory::Syntax, number: 2 },
                location: Location::new(file_path.to_string(), span),
                message: format!("UnrecognizedToken '{:?}'", token),
                expected: expected_strs,
                help: None,
            }
        }
        ParseError::ExtraToken { token: (start, token, _end) } => SprsError::Parse {
            code: ErrorCode { category: ErrorCategory::Syntax, number: 3 },
            location: Location::new(file_path.to_string(), Span::new(start, start)),
            message: format!("ExtraToken '{:?}'", token),
            expected: vec![],
            help: None,
        },
        ParseError::UnrecognizedEof { location, expected } => {
            let expected_strs: Vec<String> = expected.iter().map(|e| format!("{:?}", e)).collect();
            SprsError::Parse {
                code: ErrorCode { category: ErrorCategory::Syntax, number: 4 },
                location: Location::new(file_path.to_string(), Span::new(location, location)),
                message: "UnrecognizedEOF".to_string(),
                expected: expected_strs,
                help: None,
            }
        }
        ParseError::User { error } => {
            // User error はメッセージ内容でコード判定
            let code = if error.contains("Invalid assignment target") {
                ErrorCode { category: ErrorCategory::Syntax, number: 5 }
            } else if error.contains("Expected IDENT token") {
                ErrorCode { category: ErrorCategory::Syntax, number: 6 }
            } else if error.contains("Expected MACRO token") {
                ErrorCode { category: ErrorCategory::Syntax, number: 6 }
            } else if error.contains("Expected NUM token") {
                ErrorCode { category: ErrorCategory::Syntax, number: 6 }
            } else if error.contains("Expected FLOAT token") {
                ErrorCode { category: ErrorCategory::Syntax, number: 6 }
            } else if error.contains("Expected StrLiteral token") {
                ErrorCode { category: ErrorCategory::Syntax, number: 6 }
            } else if error.contains("does not support struct init syntax") {
                ErrorCode { category: ErrorCategory::Syntax, number: 7 }
            } else {
                ErrorCode { category: ErrorCategory::Syntax, number: 6 }
            };
            SprsError::Parse {
                code,
                location: Location::new(file_path.to_string(), Span::DUMMY),
                message: error,
                expected: vec![],
                help: None,
            }
        }
    }
}
```

- [ ] **Step 2: parser.rs の parse_only 戻り値を変更**

`src/llvm/parser.rs` を書き換え:

```rust
use crate::front::ast;
use crate::front::error::SprsError;
use crate::front::lexer;
use crate::grammar;
use crate::llvm::error_helper;

pub fn parse_only(input: &str, file_path: &str) -> Result<Vec<ast::Item>, SprsError> {
    let mut lex = lexer::Lexer::new(input);
    match grammar::StartParser::new().parse(&mut lex) {
        Ok(items) => Ok(items),
        Err(e) => Err(error_helper::to_sprs_error(input, file_path, e)),
    }
}
```

- [ ] **Step 3: ビルド確認**

Run: `cargo build 2>&1 | tail -20`
Expected: `load_and_compile_module` が `Result<_, String>` を返しているため、`?` で型不一致エラーが出る。これは Task 4 で修正する

- [ ] **Step 4: Commit**

```bash
git add src/llvm/error_helper.rs src/llvm/parser.rs
git commit -m "feat: parse error を SprsError::Parse に変換（Phase 1）"
```

---

## Task 4: module_loader と llvm_executer の SprsError 対応

**Files:**
- Modify: `src/llvm/module_loader.rs`
- Modify: `src/llvm/llvm_executer.rs`

**Interfaces:**
- Consumes: `SprsError` from `src/front/error.rs`, `parse_only` returning `Result<_, SprsError>`
- Produces: `load_and_compile_module` returning `Result<_, SprsError>`, `build_and_run` handling `SprsError`

- [ ] **Step 1: module_loader.rs の戻り値を変更**

`src/llvm/module_loader.rs` L16-20 のシグネチャを変更:

```rust
pub fn load_and_compile_module(
    &mut self,
    module_name: &str,
    main_path: Option<&String>,
) -> Result<(), SprsError> {
```

L33-34 の `.map_err(|e| format!(...))?` を変更:

```rust
let source = std::fs::read_to_string(&path).map_err(|e| {
    SprsError::Semantic {
        code: ErrorCode { category: ErrorCategory::Semantic, number: 10 },
        location: Location::new(path.clone(), Span::DUMMY),
        message: format!("Failed to read module file {}: {}", path, e),
        help: None,
    }
})?;
```

ファイル先頭に use 追加:

```rust
use crate::front::error::{SprsError, ErrorCode, ErrorCategory, Location};
use crate::front::span::Span;
```

- [ ] **Step 2: llvm_executer.rs のエラー出力を変更**

`src/llvm/llvm_executer.rs` L75-76 を変更:

```rust
if let Err(e) = compiler.load_and_compile_module("main", Some(&path)) {
    let source = std::fs::read_to_string(&path).unwrap_or_default();
    let rendered = crate::front::error::render(&e, error_format, &source);
    eprintln!("{}", rendered);
    return Err(format!("Compile Error: {}", e).into());
}
```

`build_and_run` のシグネチャに `error_format: ErrorFormat` 引数を追加。`ErrorFormat` と `SprsError` の use を追加:

```rust
use crate::front::error::{ErrorFormat, SprsError};
```

- [ ] **Step 3: ビルド確認**

Run: `cargo build 2>&1 | tail -20`
Expected: `build_and_run` の呼び出し元（main.rs）で `error_format` 引数が足りないエラー。Task 5 で修正

- [ ] **Step 4: Commit**

```bash
git add src/llvm/module_loader.rs src/llvm/llvm_executer.rs
git commit -m "feat: module_loader と llvm_executer を SprsError 対応（Phase 1）"
```

---

## Task 5: --error-format フラグ追加

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `ErrorFormat` from `src/front/error.rs`, `build_and_run` with `error_format` param
- Produces: `--error-format <json|human>` CLI フラグ

- [ ] **Step 1: main.rs に --error-format 解析を追加**

`src/main.rs` L380-403 の `build`/`run`/`debug` ブロックを変更:

```rust
"build" | "run" | "debug" => {
    let mut dest: Option<String> = None;
    let mut error_format = crate::front::error::ErrorFormat::Human;
    if argc > 2 {
        let mut iter = argv[2..].iter();
        while let Some(arg) = iter.next() {
            if arg == "--dest" {
                dest = iter.next().cloned();
                if dest.is_none() {
                    eprintln!("Usage: {} {} --dest <path>", naming::LANG_NAME, command);
                    return Err("missing value for --dest".into());
                }
            } else if arg == "--error-format" {
                let fmt_str = iter.next().cloned();
                match fmt_str {
                    Some(s) => {
                        error_format = crate::front::error::ErrorFormat::from_str(&s)
                            .map_err(|e| e.into())?;
                    }
                    None => {
                        eprintln!("Usage: {} {} --error-format <json|human>", naming::LANG_NAME, command);
                        return Err("missing value for --error-format".into());
                    }
                }
            } else {
                eprintln!("Unknown argument: {}", arg);
                return Err(format!("invalid argument: {}", arg).into());
            }
        }
    }
    let mode = match command.as_str() {
        "build" => llvm_executer::ExecuteMode::Build,
        "run" => llvm_executer::ExecuteMode::Run,
        "debug" => llvm_executer::ExecuteMode::Debug,
        _ => unreachable!(),
    };
    llvm_executer::build_and_run(dest.as_deref(), mode, error_format)?;
    Ok(())
}
```

- [ ] **Step 2: ビルド確認**

Run: `cargo build 2>&1 | tail -5`
Expected: ビルド成功

- [ ] **Step 3: スモークテスト**

正常系:
```bash
cargo run -- run --dest tests 2>&1 | tail -5
```
Expected: テストスイートが通常通り実行される（87 PASS / 7 XFAIL / 8 FAIL）

エラー系（JSON）:
```bash
echo 'fn main() { var x = ; }' > /tmp/test_err.sprs
cargo run -- run --error-format json 2>&1 | head -5
rm /tmp/test_err.sprs
```
Expected: JSON 形式のエラー出力

エラー系（human）:
```bash
echo 'fn main() { var x = ; }' > /tmp/test_err.sprs
cargo run -- run --error-format human 2>&1 | head -10
rm /tmp/test_err.sprs
```
Expected: Rust 風の人間可読エラー出力

- [ ] **Step 4: 全テストスイート実行**

Run: `cargo run -- run --dest tests 2>&1`
Expected: 87 PASS / 7 XFAIL / 8 FAIL / 0 クラッシュ（回帰なし）

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/llvm/llvm_executer.rs
git commit -m "feat: --error-format フラグ追加（Phase 1 完了）"
```

---

# Phase 2: コンパイル時エラー構造化 + AST span 基盤工事

## Task 6: Compiler に sources フィールド追加

advisory 指摘の source 伝達問題対応。codegen 中にエラーが発生した際、元ソース文字列を参照して snippet/line/col を描画できるようにする。

**Files:**
- Modify: `src/llvm/compiler.rs`

**Interfaces:**
- Produces: `Compiler.sources: HashMap<String, String>` (モジュール名 → ソース本文)

- [ ] **Step 1: Compiler 構造体に sources フィールド追加**

`src/llvm/compiler.rs` L29-42 に `sources` フィールドを追加:

```rust
pub struct Compiler<'ctx> {
    pub context: &'ctx Context,
    pub modules: HashMap<String, Module<'ctx>>,
    pub builder: Builder<'ctx>,
    pub scopes: Vec<Scope<'ctx>>,
    pub function_signatures: Option<FunctionValue<'ctx>>,
    pub runtime_value_type: StructType<'ctx>,
    pub target_os: OS,
    pub string_counter: usize,
    pub malloc_type: inkwell::types::FunctionType<'ctx>,
    pub source_path: String,
    pub struct_defs: HashMap<String, StructDef<'ctx>>,
    pub enum_names: HashSet<String>,
    pub sources: HashMap<String, String>, // module name → source text
}
```

- [ ] **Step 2: Compiler::new で sources 初期化**

`Compiler::new` の初期化部分に `sources: HashMap::new(),` を追加。

- [ ] **Step 3: module_loader.rs で source を保存**

`src/llvm/module_loader.rs` の `load_and_compile_module` L33-36 で、`source` を `self.sources` に保存:

```rust
let source = std::fs::read_to_string(&path).map_err(|e| {
    SprsError::Semantic {
        code: ErrorCode { category: ErrorCategory::Semantic, number: 10 },
        location: Location::new(path.clone(), Span::DUMMY),
        message: format!("Failed to read module file {}: {}", path, e),
        help: None,
    }
})?;

// source を Compiler に保存（エラー出力時の snippet 描画用）
self.sources.insert(module_name.to_string(), source.clone());

let items = parse_only(&source, &path)?;
```

- [ ] **Step 4: ビルド確認**

Run: `cargo build 2>&1 | tail -5`
Expected: ビルド成功

- [ ] **Step 5: Commit**

```bash
git add src/llvm/compiler.rs src/llvm/module_loader.rs
git commit -m "feat: Compiler に sources フィールド追加（Phase 2 基盤）"
```

---

## Task 7: AST を Spanned<T> で包む

**注意**: この Task は AST 全体の構造を変えるため、コンパイルエラーが大量に出る。Task 8-10 で段階的に修正する。

**Files:**
- Modify: `src/front/ast.rs`

**Interfaces:**
- Consumes: `Span`, `Spanned` from `src/front/span.rs`
- Produces: AST 全体が `Spanned<...>` で包まれた状態

- [ ] **Step 1: ast.rs に use 追加**

`src/front/ast.rs` の先頭に追加:

```rust
use crate::front::span::{Span, Spanned};
```

- [ ] **Step 2: Expr の再帰的フィールドを Box<Spanned<Expr>> に変更**

`src/front/ast.rs` L4-48 の `Expr` enum を変更。`Box<Expr>` を `Box<Spanned<Expr>>` に、`Vec<Expr>` を `Vec<Spanned<Expr>>` に:

```rust
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Number(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Add(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Mul(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Minus(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Div(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Mod(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Eq(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Neq(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Lt(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Gt(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Le(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Ge(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    If(Box<Spanned<Expr>>, Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Call(String, Vec<Spanned<Expr>>, Option<Type>),
    Var(String),
    Increment(Box<Spanned<Expr>>),
    Decrement(Box<Spanned<Expr>>),
    Neg(Box<Spanned<Expr>>),
    List(Vec<Spanned<Expr>>),
    Range(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Index(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    ModuleAccess(String, String, Vec<Spanned<Expr>>),
    FieldAccess(Box<Spanned<Expr>>, String),
    Unit(),
    Macro(String, Vec<Spanned<Expr>>),
    StructInit(String, Vec<(String, Spanned<Expr>)>),

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
```

- [ ] **Step 3: FunctionParam, Function, VarDecl, AssignStmt を Spanned で包む**

これらの構造体は `Spanned<...>` に包むのではなく、フィールドとして `span: Span` を持たせる（構造体なのでラッパーよりフィールドの方が自然）。ただし、grammar から生成する際は `Spanned<FunctionParam>` として包む。

**設計判断**: 構造体（Function, VarDecl, AssignStmt, Enum, Struct, StructField, FunctionParam）はフィールドに `span: Span` を追加する方針に変更。enum（Expr, Stmt, Item）は `Spanned<T>` ラッパーで包む。

```rust
#[derive(Debug, PartialEq)]
pub struct FunctionParam {
    pub ident: String,
    pub ty: Option<Type>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub ident: String,
    pub params: Vec<FunctionParam>,
    pub blk: Vec<Spanned<Stmt>>,
    pub is_public: bool,
    pub ret_ty: Option<Type>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct VarDecl {
    pub ident: String,
    pub expr: Option<Spanned<Expr>>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct AssignStmt {
    pub name: String,
    pub expr: Spanned<Expr>,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct Enum {
    pub ident: String,
    pub variants: Vec<String>,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct Struct {
    pub ident: String,
    pub fields: Vec<StructField>,
    pub _methods: Vec<Function>,
    pub is_public: bool,
    pub span: Span,
}

#[derive(Debug, PartialEq, Clone)]
pub struct StructField {
    pub ident: String,
    pub ty: Option<Type>,
    pub default_value: Option<Spanned<Expr>>,
    pub span: Span,
}
```

- [ ] **Step 4: Stmt enum を Spanned<Stmt> 対応に**

```rust
#[derive(Debug, PartialEq)]
pub enum Stmt {
    Var(VarDecl),
    Assign(AssignStmt),
    Expr(Spanned<Expr>),
    If {
        cond: Spanned<Expr>,
        then_blk: Vec<Spanned<Stmt>>,
        else_blk: Option<Vec<Spanned<Stmt>>>,
    },
    While {
        cond: Spanned<Expr>,
        body: Vec<Spanned<Stmt>>,
    },
    Return(Option<Spanned<Expr>>),
    EnumItem(Enum),
}
```

- [ ] **Step 5: Item enum を Spanned<Item> 対応に**

```rust
#[derive(Debug, PartialEq)]
pub enum Item {
    Import(String),
    Package(String),
    VarItem(VarDecl),
    FunctionItem(Function),
    Preprocessor(String),
    EnumItem(Enum),
    StructItem(Struct),
}
```

- [ ] **Step 6: ビルド確認（エラー多数を想定）**

Run: `cargo build 2>&1 | grep "^error" | wc -l`
Expected: 大量のエラー（grammar.lalrpop と codegen が未対応のため）。これは Task 8-10 で修正する

- [ ] **Step 7: Commit**

```bash
git add src/front/ast.rs
git commit -m "refactor: AST を Spanned<T> で包む（Phase 2、コンパイルエラーは後続Taskで修正）"
```

---

## Task 8: grammar.lalrpop に @L/@R マーカー追加と Spanned 生成

この Task は grammar 全規則（約40）に `@L`/`@R` マーカーを追加し、action 内で `Spanned::new(node, span)` を生成する。機械的だが量大。

**Files:**
- Modify: `src/grammar.lalrpop`
- Regenerate: `src/grammar.rs` (touch src/grammar.lalrpop で自動再生成)

**Interfaces:**
- Consumes: `Span`, `Spanned` from `src/front/span.rs`
- Produces: grammar が `Spanned<Expr>`, `Spanned<Stmt>`, `Spanned<Item>` 等を生成

- [ ] **Step 1: grammar.lalrpop に use 追加**

`src/grammar.lalrpop` L3-15 の use ブロックに `Span`, `Spanned` を追加:

```rust
use crate::front::ast::{
    Item, 
    VarDecl, 
    Expr, 
    Stmt, 
    Function, 
    FunctionParam, 
    Enum, 
    AssignStmt,
    Struct,
    StructField,
    Suffix,
};
use crate::front::span::{Span, Spanned};
use crate::front::lexer::Token;
use crate::front::type_helper::Type;
use lalrpop_util::ParseError;
use half::f16;
```

- [ ] **Step 2: Item 系規則に @L/@R 追加**

各規則の先頭に `<start:@L>`、末尾に `<end:@R>` を追加。例:

```rust
pub Start: Vec<Spanned<Item>> =
    <items:ItemNode*> => items;

ItemNode: Spanned<Item> = {
    <start:@L> <v:VarDecl> <end:@R> => Spanned::new(Item::VarItem(v), Span::new(start, end)),
    <start:@L> <p:PreprocessorDirective> <end:@R> => Spanned::new(Item::Preprocessor(p), Span::new(start, end)),
    <start:@L> <i:ImportDirective> <end:@R> => Spanned::new(Item::Import(i), Span::new(start, end)),
    <start:@L> <p:PackageDirective> <end:@R> => Spanned::new(Item::Package(p), Span::new(start, end)),
    <start:@L> <e:EnumDef> <end:@R> => Spanned::new(Item::EnumItem(e), Span::new(start, end)),
    <start:@L> <s:StructDef> <end:@R> => Spanned::new(Item::StructItem(s), Span::new(start, end)),
    FunctionDef,
};
```

- [ ] **Step 3: EnumDef, StructDef に span フィールド設定**

```rust
EnumDef: Enum = 
    <start:@L> <is_pub:PublicKw> Enum <name:Ident> LBrace <variants:EnumVariantList> RBrace <end:@R> => {
        Enum {
            ident: name,
            variants,
            is_public: is_pub,
            span: Span::new(start, end),
        }
    };

StructDef: Struct =
    <start:@L> <is_pub:PublicKw> Struct <name:Ident> LBrace <fields:StructFieldList> RBrace <end:@R> => {
        Struct {
            ident: name,
            fields,
            is_public: is_pub,
            _methods: vec![],
            span: Span::new(start, end),
        }
};
```

- [ ] **Step 4: FunctionDef, VarDecl, AssignStmt に span 設定**

```rust
FunctionDef: Spanned<Item> =
   <start:@L> <is_pub:PublicKw> FnKw <name:Ident> LParen <params:ParamList> RParen <ret:ReturnType> <body:Block> <end:@R> => {
        Spanned::new(Item::FunctionItem(Function {
            ident: name,
            params,
            ret_ty: ret,
            blk: body,
            is_public: is_pub,
            span: Span::new(start, end),
        }), Span::new(start, end))
    };

VarDecl: VarDecl = {
    <start:@L> Var <id:Ident> Assign <e:Expr> Semi <end:@R> => VarDecl { ident: id, expr: Some(e), span: Span::new(start, end) },
    <start:@L> Var <id:Ident> Semi <end:@R> => VarDecl { ident: id, expr: None, span: Span::new(start, end) },
};
```

- [ ] **Step 5: Stmt 規則に @L/@R 追加**

```rust
Stmt: Spanned<Stmt> = {
    <start:@L> <v:VarDecl> <end:@R> => Spanned::new(Stmt::Var(v), Span::new(start, end)),
    <start:@L> <e:Expr> <tail:StmtTail> <end:@R> =>? {
        match tail {
            None => Ok(Spanned::new(Stmt::Expr(e), Span::new(start, end))),
            Some(val) => {
                if let Expr::Var(id) = e.node {
                    Ok(Spanned::new(Stmt::Assign(AssignStmt { name: id, expr: val, span: Span::new(start, end) }), Span::new(start, end)))
                } else {
                    Err(ParseError::User { error: "Invalid assignment target".to_string() })
                }
            }
        }
    },
    IfStmt,
    <start:@L> While <c:Expr> <body:Block> <end:@R> =>
        Spanned::new(Stmt::While { cond: c, body: body }, Span::new(start, end)),
    <start:@L> Return <e:Expr> Semi <end:@R> => Spanned::new(Stmt::Return(Some(e)), Span::new(start, end)),
    <start:@L> Return Semi <end:@R> => Spanned::new(Stmt::Return(None), Span::new(start, end)),
}

IfStmt: Spanned<Stmt> = {
    <start:@L> If <c:Expr> <then:Block> <end:@R> => Spanned::new(Stmt::If {
        cond: c,
        then_blk: then,
        else_blk: None,
    }, Span::new(start, end)),
    <start:@L> If <c:Expr> <then:Block> Else <else_blk:Block> <end:@R> => Spanned::new(Stmt::If {
        cond: c,
        then_blk: then,
        else_blk: Some(else_blk),
    }, Span::new(start, end)),
};
```

- [ ] **Step 6: Expr 系規則に @L/@R 追加**

全 Expr 規則（RangeExpr, Comparison, AddAndMinus, MulAndDivAndMod, Unary, Postfix, Atom）に `@L`/`@R` を追加し、`Spanned::new(Expr::..., Span::new(start, end))` で包む。

例（Comparison）:
```rust
Comparison: Spanned<Expr> = {
    <start:@L> <l:Comparison> EqEq <r:AddAndMinus> <end:@R> => Spanned::new(Expr::Eq(Box::new(l), Box::new(r)), Span::new(start, end)),
    <start:@L> <l:Comparison> Neq <r:AddAndMinus> <end:@R> => Spanned::new(Expr::Neq(Box::new(l), Box::new(r)), Span::new(start, end)),
    // ... 同様に Lt, Gt, Le, Ge
    <a:AddAndMinus> => a,
}
```

Atom 規則も全て `Spanned::new(...)` で包む:
```rust
Atom: Spanned<Expr> = {
    <start:@L> <id:Ident> <end:@R> => Spanned::new(Expr::Var(id), Span::new(start, end)),
    <start:@L> <n:Num> <end:@R> => Spanned::new(Expr::Number(n), Span::new(start, end)),
    // ... 全 Atom 規則
}
```

- [ ] **Step 7: grammar.rs 再生成**

```bash
touch src/grammar.lalrpop
cargo build 2>&1 | tail -20
```

Expected: grammar.rs が再生成され、grammar 関連のエラーは減る。codegen のエラーは Task 9 で修正。

- [ ] **Step 8: Commit**

```bash
git add src/grammar.lalrpop
git commit -m "refactor: grammar に @L/@R マーカー追加と Spanned 生成（Phase 2）"
```

---

## Task 9: codegen のパターンマッチを Spanned<Expr> 対応に

**Files:**
- Modify: `src/llvm/codegen.rs`

**Interfaces:**
- Consumes: `Spanned<Expr>`, `Spanned<Stmt>` from AST
- Produces: `compile_expr(&mut self, expr: &Spanned<Expr>, ...)` 等のシグネチャ

- [ ] **Step 1: compile_expr のシグネチャ変更**

`src/llvm/codegen.rs` L420 の `compile_expr` シグネチャを変更:

```rust
pub(crate) fn compile_expr(
    &mut self,
    expr: &Spanned<Expr>,
    module: &Module<'ctx>,
) -> Result<PointerValue<'ctx>, SprsError> {
    let expr_inner = &expr.node;
    match expr_inner {
        // 既存のパターンマッチ。Box<Expr> → Box<Spanned<Expr>> に変更済みなので
        // パターンはそのまま動くはず
        ...
    }
}
```

- [ ] **Step 2: get_known_type_from_expr, infer_type, get_expr_name のシグネチャ変更**

```rust
pub fn get_known_type_from_expr(&self, expr: &Spanned<Expr>) -> Result<String, SprsError> {
    match &expr.node {
        ...
    }
}

fn infer_type(&self, expr: &Spanned<Expr>) -> Type {
    match &expr.node {
        ...
    }
}

pub fn get_expr_name(&self, expr: &Spanned<Expr>) -> Option<String> {
    match &expr.node {
        ...
    }
}
```

- [ ] **Step 3: compile_fn, compile_return, compile_block のシグネチャ変更**

```rust
pub fn compile_fn(
    &mut self,
    func: &Function,
    module: &Module<'ctx>,
) -> Result<(), SprsError> {
    ...
}
```

`compile_block` の `stmts: &Vec<Stmt>` を `stmts: &Vec<Spanned<Stmt>>` に変更。

- [ ] **Step 4: ビルド確認**

Run: `cargo build 2>&1 | grep "^error" | head -20`
Expected: codegen 内部のパターンマッチエラー。`expr.node` で内部を取り出す箇所を順次修正

- [ ] **Step 5: パターンマッチを順次修正**

コンパイラのエラーメッセージに従い、`match expr { ... }` → `match &expr.node { ... }` に変更。`Box<Expr>` → `Box<Spanned<Expr>>` に対応。

- [ ] **Step 6: ビルド成功確認**

Run: `cargo build 2>&1 | tail -5`
Expected: ビルド成功（警告は許容）

- [ ] **Step 7: Commit**

```bash
git add src/llvm/codegen.rs
git commit -m "refactor: codegen を Spanned<Expr> 対応に（Phase 2）"
```

---

## Task 10: 全 Result<_, String> を Result<_, SprsError> に置換

**Files:**
- Modify: `src/llvm/compiler.rs`, `arithmetic.rs`, `data_structures.rs`, `macros.rs`, `value.rs`, `variable.rs`, `comparison.rs`, `control_flow.rs`, `module_loader.rs`

**Interfaces:**
- Consumes: `SprsError`, `ErrorCode`, `ErrorCategory`, `Location` from `src/front/error.rs`
- Produces: 全 codegen 関数が `Result<_, SprsError>` を返す

- [ ] **Step 1: 各ファイルの use に SprsError 系を追加**

各ファイルの先頭に追加:
```rust
use crate::front::error::{SprsError, ErrorCode, ErrorCategory, Location};
use crate::front::span::Span;
```

- [ ] **Step 2: Err(format!(...)) を Err(SprsError::Semantic { ... }) に置換**

各エラーサイトで、対応するエラーコード（spec セクション6カタログ参照）を指定:

例（codegen.rs L457 の Undefined variable）:
```rust
Err(SprsError::Semantic {
    code: ErrorCode { category: ErrorCategory::Semantic, number: 2 },
    location: Location::new(self.current_file.clone(), expr.span),
    message: format!("Undefined variable: {}", ident),
    help: Some("変数を使用する前に var で宣言してください。".to_string()),
})
```

各エラーサイトのコード番号は spec セクション6カタログに従う:
- SPRS-SEM-001: Unknown type expression
- SPRS-SEM-002: Undefined variable
- SPRS-SEM-003: Unknown macro
- SPRS-SEM-004: Undefined enum variant
- SPRS-SEM-005: struct initialization requires @init
- SPRS-SEM-006: Unknown runtime function
- SPRS-SEM-007: Field index out of bounds
- SPRS-SEM-008: @cast second argument
- SPRS-SEM-009: Unsupported target type for @cast
- SPRS-SEM-010: Failed to read module file
- SPRS-SEM-011: Failed to get panic error string constant
- SPRS-SEM-012: create_entry_block_alloca 内部エラー
- SPRS-TYP-001: Type mismatch (pointer/str)
- SPRS-TYP-002: Type mismatch (Bool)
- SPRS-TYP-003: Type mismatch (Int)
- SPRS-TYP-004: Type mismatch (Float)

- [ ] **Step 3: location フィールドに span を設定**

各エラーサイトで、対応する AST ノードの `span` を `Location` に設定。`self.current_file` または `self.source_path` でファイル名を取得。

**注意**: `current_file` フィールドが Compiler に無い場合、`source_path` を使用。複数モジュールの場合は `module_loader.rs` で `self.current_module_name` を追跡する必要がある。

- [ ] **Step 4: ビルド確認**

Run: `cargo build 2>&1 | tail -10`
Expected: ビルド成功

- [ ] **Step 5: スモークテスト**

Run: `cargo run -- run --dest tests 2>&1`
Expected: 87 PASS / 7 XFAIL / 8 FAIL / 0 クラッシュ（回帰なし）

- [ ] **Step 6: JSON エラー出力確認**

```bash
echo 'fn main() { @println(undefined_var); }' > /tmp/test_sem.sprs
cargo run -- run --error-format json 2>&1 | head -5
rm /tmp/test_sem.sprs
```
Expected: SPRS-SEM-002 の JSON エラー出力

- [ ] **Step 7: Commit**

```bash
git add src/llvm/
git commit -m "feat: 全 Result<_, String> を Result<_, SprsError> に置換（Phase 2 完了）"
```

---

## Task 11: error_helper.rs リネームと最終クリーンアップ

**Files:**
- Rename: `src/llvm/error_helper.rs` → `src/front/error_reporter.rs`
- Modify: `src/llvm/mod.rs`, `src/front/mod.rs`

- [ ] **Step 1: error_helper.rs を error_reporter.rs にリネーム**

```bash
git mv src/llvm/error_helper.rs src/front/error_reporter.rs
```

- [ ] **Step 2: モジュール宣言を更新**

`src/front/mod.rs` に追加:
```rust
pub mod error_reporter;
```

`src/llvm/mod.rs` から `pub mod error_helper;` を削除。

- [ ] **Step 3: import パスを更新**

`src/llvm/parser.rs` の import を変更:
```rust
use crate::front::error_reporter;
```

- [ ] **Step 4: ビルド確認**

Run: `cargo build 2>&1 | tail -5`
Expected: ビルド成功

- [ ] **Step 5: 全テストスイート実行**

Run: `cargo run -- run --dest tests 2>&1`
Expected: 87 PASS / 7 XFAIL / 8 FAIL / 0 クラッシュ

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: error_helper を front/error_reporter にリネーム（最終クリーンアップ）"
```

---

## Plan Self-Review

### Spec カバレッジ
- ✅ Phase 1: parse error 構造化（Task 1-5）
- ✅ Phase 2: AST span 基盤工事（Task 6-7）
- ✅ Phase 2: grammar @L/@R マーカー（Task 8）
- ✅ Phase 2: codegen Spanned 対応（Task 9）
- ✅ Phase 2: Result<_, SprsError> 置換（Task 10）
- ✅ クリーンアップ（Task 11）
- ✅ advisory 指摘の source 伝達問題（Task 6）
- ✅ CLI --error-format フラグ（Task 5）

### プレースホルダスキャン
- Task 8 Step 6「全 Expr 規則に @L/@R 追加」は詳細を省略しているが、Comparison の例があれば機械的に適用可能。実装時に全規則を網羅することを明記。

### 型整合性
- `Spanned<T>` は Task 1 で定義、Task 7-10 で使用。型名は一貫。
- `SprsError` は Task 2 で定義、Task 3-10 で使用。バリアント名は一貫。
- `ErrorCode` の番号は spec セクション6カタログと一致。

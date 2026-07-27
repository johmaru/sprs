# 構造化エラーレポート設計 (Structured Error Reporting)

**作成日**: 2026-07-27
**ステータス**: Draft (ユーザー承認待ち)
**対象フェーズ**: Phase 1 (parse error) + Phase 2 (意味/型エラー)

---

## 1. 目的

sprs を書く AI agent（Claude Code 等）がエラーを機械的に解釈し、修正案を精度よく出せるようにする。

人間ユーザーが AI とペアプロする、または AI agent 自体が sprs コードを生成する場面で、エラーメッセージが構造化データとして出力されることで、AI は以下が可能になる:

- エラーの**位置**（file/line/col）から該当ソース行を直接特定
- エラーの**意味**（安定ID code + category）で既知のパターンにマッチ
- **修正ヒント**（help / doc_ref）で具体的な修正方向を即座に提示
- **文脈値**（context、将来 Phase 3b）で「どの値が悪かったか」を機械取得

---

## 2. 対象外（scope 外）

- **実行時 panic の `context`（実行時値ダンプ）**: Phase 3b。issue #26（catchable エラー機構、`Tag::Error` vs `Result<T,E>`）の設計決着に依存するため本 spec の対象外。
- **実行時 panic の `location`**: Phase 3a。AST span 完了後に `__panic` ABI 拡張（`message_ptr` に加え file/line/col を静的引数として埋め込む）で実現可能。issue #26 非依存だが、本 spec では「Phase 2 完了後の拡張点」として文書化するのみで実装しない。
- **LLVM debug metadata**: 実行時デバッガ用の重い仕組み。静的引数埋め込みで十分なため導入しない。
- **静的解析全般**: sprs はデフォルト動的型付け言語のため、borrow チェック等の高度な静的解析エラーは対象外。

---

## 3. 全体アーキテクチャ

```
sprs source ─► [lexer] ─► (usize, Token, usize) ─► [lalrpop parser] ─► Vec<Spanned<Item>>
                                                                       │
                                          ┌────────────────────────────┤
                                          ▼                            ▼
                                  [parse error]               [compile error]
                                  SprsError::Parse            SprsError::Semantic / Type
                                          │                            │
                                          └─────────┬──────────────────┘
                                                    ▼
                                          [error reporter]
                                            - human (default)
                                            - json (--error-format=json)
```

### 3.1 データフロー

1. **lexer** (`src/front/lexer.rs`): 現状 `(usize, Token, usize)` を返す。変更なし。
2. **lalrpop parser** (`src/grammar.lalrpop`): AST に `Span` を thread するよう全規則を書き換え。`Spanned<T>` で包む。
3. **AST** (`src/front/ast.rs`): `Spanned<T>` ラッパーを新設。`Expr` → `Spanned<Expr>`、`Stmt` → `Spanned<Stmt>`、`Item` → `Spanned<Item>`。
4. **codegen** (`src/llvm/codegen.rs` 他): `Result<_, String>` を `Result<_, SprsError>` に全面置換。エラーサイトで span と category を付与。
5. **error reporter** (`src/llvm/error_helper.rs` を拡張、新設 `src/front/error.rs`): `SprsError` を人間可読 or JSON で出力。

---

## 4. 主要データ構造

### 4.1 `Span`（位置情報）

```rust
// src/front/ast.rs に新設

/// ソースコード上の範囲をバイトオフセットで表現。
/// lexer.rs:227 の logos::span と互換。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// 空スパン（位置不明・合成ノード用）
    pub const DUMMY: Span = Span { start: 0, end: 0 };
}
```

**設計判断**: バイトオフセットのみ保持し、line/col は出力時に `get_line_col` で変換（現状 `error_helper.rs:55` の実装を再利用）。AST 全ノードに2つの `usize` が乗るが、`Spanned<T>` で包むため `T` が大きいノードでは相対的に無視できる。

### 4.2 `Spanned<T>` ラッパー

```rust
// src/front/ast.rs に新設

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
```

**適用対象**:
- `Expr` → `Spanned<Expr>`（再帰的フィールド `Box<Expr>` は `Box<Spanned<Expr>>` に）
- `Stmt` → `Spanned<Stmt>`
- `Item` → `Spanned<Item>`
- `Function` / `VarDecl` / `AssignStmt` / `Enum` / `Struct` / `StructField` / `FunctionParam` → それぞれ `Spanned<...>` に

**適用しないもの**:
- `Type` enum: 位置情報不要。型名トークンは式として扱う `Expr::TypeI8` 等で span が取れるため、`Type` 自体は span を持たない。`FunctionParam` の `ty: Option<Type>` で型エラーが出た際は、親の `Spanned<FunctionParam>` の span で代用する
- `Suffix`: 現状未使用（`_methods: Vec<Function>` のみ）のため後回し


### 4.3 `SprsError` enum

```rust
// src/front/error.rs に新設

use crate::front::ast::Span;

/// 安定エラーコード。仕様変更で変わらない ID。
/// 形式: SPRS-<CAT>-<NNN>
/// CAT: SYN (Syntax) | TYP (Type) | SEM (Semantic) | RUN (Runtime、将来)
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
    /// 意味エラー: 未定義変数、未定義関数、未知のマクロ、未知の enum variant 等
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

#[derive(Debug, Clone)]
pub enum SprsError {
    /// lalrpop の ParseError を構造化したもの
    Parse {
        code: ErrorCode,
        location: Location,
        message: String,
        expected: Vec<String>,  // UnrecognizedToken/UnrecognizedEof 用
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
```

---

## 5. 出力スキーマ

### 5.1 JSON 出力（`--error-format=json`）

```json
{
  "code": "SPRS-SEM-003",
  "category": "Semantic",
  "phase": "compile",
  "severity": "error",
  "message": "Undefined variable: x",
  "location": {
    "file": "main.sprs",
    "line": 42,
    "column": 18,
    "end_line": 42,
    "end_column": 19,
    "snippet": "    @println(x)"
  },
  "context": {
    "expected": null,
    "actual": null
  },
  "help": "変数 x を使用する前に var x = ... で宣言してください。",
  "doc_ref": "sprs://errors/SPRS-SEM-003"
}
```

### 5.2 人間可読出力（デフォルト）

```
error[SPRS-SEM-003]: Undefined variable: x
  --> main.sprs:42:18
   |
42 |     @println(x)
   |                  ^
help: 変数 x を使用する前に var x = ... で宣言してください。
doc:  sprs://errors/SPRS-SEM-003
```

Rust のエラー表示スタイルを踏襲。AI agent は JSON モードを使うことを想定。

---

## 6. エラーコードカタログ（初期登録分）

### 6.1 Syntax カテゴリ (SPRS-SYN-NNN)

| コード | lalrpop variant | 内容 |
|---|---|---|
| SPRS-SYN-001 | InvalidToken | 字句解析で無効なトークン |
| SPRS-SYN-002 | UnrecognizedToken | 予期しないトークン |
| SPRS-SYN-003 | ExtraToken | 不要なトークン |
| SPRS-SYN-004 | UnrecognizedEof | ファイル終端でパース未完了 |
| SPRS-SYN-005 | User (Invalid assignment target) | 代入先が変数でない |
| SPRS-SYN-006 | User (Expected IDENT/MACRO/NUM/FLOAT/StrLiteral token) | トークン種別ミスマッチ |
| SPRS-SYN-007 | User (Macro does not support struct init syntax) | @init 以外での struct init 構文 |

### 6.2 Type カテゴリ (SPRS-TYP-NNN)

| コード | 既存メッセージ | 内容 |
|---|---|---|
| SPRS-TYP-001 | "Type mismatch: Function expects pointer type (e.g. str) but got ..." | str 戻り値型注釈への非ポインタ型 return |
| SPRS-TYP-002 | "Type mismatch: Function expects Bool but got ..." | bool 戻り値型注釈への非 bool return |
| SPRS-TYP-003 | "Type mismatch: Function expects Int type but got ..." | int 戻り値型注釈への非 int return |
| SPRS-TYP-004 | "Type mismatch: Function expects Float type but got ..." | float 戻り値型注釈への非 float return |

### 6.3 Semantic カテゴリ (SPRS-SEM-NNN)

| コード | 既存メッセージ | 内容 |
|---|---|---|
| SPRS-SEM-001 | "Unknown type expression for known type: ..." | get_known_type_from_expr の未知パターン |
| SPRS-SEM-002 | "Undefined variable: ..." | 未定義変数参照 |
| SPRS-SEM-003 | "Unknown macro: ..." | 未知のマクロ呼び出し |
| SPRS-SEM-004 | "Undefined enum variant: ..." | 未知の enum variant |
| SPRS-SEM-005 | "struct initialization requires @init(...) syntax" | @init 構文エラー |
| SPRS-SEM-006 | "Unknown runtime function: ..." | get_runtime_fn の未知関数 |
| SPRS-SEM-007 | "Field index out of bounds for struct ..." | 構造体フィールド範囲外 |
| SPRS-SEM-008 | "@cast second argument must be a type identifier" | @cast 第2引数型エラー |
| SPRS-SEM-009 | "Unsupported target type for @cast: ..." | @cast 未知ターゲット型 |
| SPRS-SEM-010 | "Failed to read module file ...: ..." | モジュールファイル読み込み失敗 |
| SPRS-SEM-011 | "Failed to get panic error string constant" | create_panic_err 内部エラー |
| SPRS-SEM-012 | "no insert block" / "no parent function" / "no entry block" | create_entry_block_alloca 内部エラー |

### 6.4 help 文のガイドライン

- **具体的**: 「型を確認してください」ではなく「`>> i64` を `>> fp` に変更してください」レベル
- **実例付き**: 可能なら修正後のコード断片を含める
- **doc_ref**: 将来的に言語内ドキュメント（`sprs://errors/SPRS-SEM-003`）への参照。Phase 1/2 では `help` のみ実装し、doc_ref はURL形式の文字列のみ（実際のドキュメント参照機構は別途）

---

## 7. 実装計画

### Phase 1: parse error 構造化（基盤工事不要）

**対象ファイル**:
- `src/llvm/error_helper.rs` — `format_parse_error` を `to_sprs_error` に置き換え、`SprsError::Parse` を生成
- `src/front/error.rs` — 新設。`SprsError`, `ErrorCode`, `Location`, `ErrorCategory` を定義
- `src/front/mod.rs` — `error` モジュールを追加
- `src/front/ast.rs` — `Span` と `Spanned<T>` の最小定義（Phase 2 で本格使用）。Phase 1 では parse error の `location` は lalrpop の `usize` から直接 `Span` を作る
- `src/llvm/parser.rs` — `parse_only` の戻り値を `Result<Vec<Item>, String>` から `Result<Vec<Item>, SprsError>` に変更
- `src/main.rs` — `--error-format` フラグ追加、エラー出力の JSON/human 切替
- `src/llvm/llvm_executer.rs` — `Compile Error: {}` の出力を `SprsError` に対応

**作業**:
1. `src/front/error.rs` を新設し、`SprsError` enum と `ErrorCode` を定義
2. `error_helper.rs` を `src/front/error.rs` 側に移動・統合。`format_parse_error` を `ParseError` → `SprsError::Parse` 変換関数に書き換え
3. `parser.rs` の `parse_only` 戻り値を `Result<Vec<Item>, SprsError>` に
4. `error_helper` を `error_reporter` に改名し、`render(error: &SprsError, format: ErrorFormat, source: &str) -> String` を実装
5. `main.rs` に `--error-format=json|human` 引数を追加（デフォルト human）
6. `llvm_executer.rs` の `load_and_compile_module` の戻り値も `Result<_, SprsError>` に

**検証**: 既存テストスイートの parse error ケース（BUG-F01/F02/F03/F10 の修正で追加されたエラーケース）で、JSON/human 両モードの出力を確認。

### Phase 2: コンパイル時エラー構造化 + AST span 基盤工事

**対象ファイル**:
- `src/front/ast.rs` — `Expr`/`Stmt`/`Item`/`Function`/`VarDecl`/`AssignStmt`/`Enum`/`Struct`/`StructField`/`FunctionParam` を `Spanned<...>` に
- `src/grammar.lalrpop` — 全規則を `Spanned<...>` 生成に書き換え。各規則に `@L`/`@R` マーカーを追加して span を取得し、action 内で `Spanned::new(node, Span { start, end })` を生成
- `src/llvm/codegen.rs` — `compile_expr`, `infer_type`, `get_known_type_from_expr`, `get_expr_name` のシグネチャ変更。`Result<_, String>` を `Result<_, SprsError>` に
- `src/llvm/compiler.rs`, `arithmetic.rs`, `data_structures.rs`, `macros.rs`, `value.rs`, `variable.rs`, `comparison.rs`, `control_flow.rs`, `module_loader.rs` — 全 `Result<_, String>` を `Result<_, SprsError>` に置換
- `src/front/error.rs` — `SprsError::Semantic` / `SprsError::Type` のコンストラクタヘルパーを追加

**作業**:
1. `Spanned<T>` を `ast.rs` に定義
2. `ast.rs` の全 enum/struct を `Spanned<...>` で包む。`Box<Expr>` → `Box<Spanned<Expr>>`
3. `grammar.lalrpop` の全規則を書き換え。lalrpop の action 内で `Spanned::new(node, span)` を生成。span は `(start, _, end)` パターンで取得
4. `grammar.rs` を再生成（`touch src/grammar.lalrpop` で lalrpop ビルド時に強制再生成）
5. codegen の全パターンマッチを `Spanned<Expr>` 対応に。`expr.node` で内部を取り出す
6. 全 `Err(format!(...))` を `Err(SprsError::Semantic { ... })` に置換。span は対応する AST ノードから取得
7. 既存 `error_helper.rs` / `error.rs` の `render` 関数で `SprsError::Semantic` / `Type` を出力

**検証**:
- 全テストスイート（87 PASS / 7 XFAIL / 8 FAIL）が通ること
- `get_known_type_from_expr` / `infer_type` のエラーケースで location が出ること
- JSON モードで `SprsError::Semantic` が出力されること

### Phase 2 のリスクと対策

| リスク | 影響 | 対策 |
|---|---|---|
| `Spanned<T>` の `Box<Spanned<Expr>>` 再帰構造のメモリレイアウト | AST 全体のメモリ増 | `Span { start, end }` は `usize` 2つ = 16バイト。`Box<Spanned<Expr>>` は `Box<Expr>` + 16バイト。許容範囲 |
| lalrpop の span 取得の工数 | grammar 全規則に `@L`/`@R` マーカー追加が必要（現状0箇所）。規則数は約40。各規則で `Span { start: @L, end: @R }` を組み立てて `Spanned::new(node, span)` に渡す。機械的だが量大 |
| codegen の全パターンマッチ書き換え | 広範囲な改修 | 段階的: まず `compile_expr` のシグネチャを変え、コンパイラにエラーを吐かせて callsite を順次修正 |
| `grammar.rs` の再生成忘れ | パースエラー | ビルド時に `lalrpop` build script が `grammar.lalrpop` のハッシュで再生成を判定。`touch` で強制可能 |

---

## 8. CLI インターフェース

### 8.1 `--error-format` フラグ

`build` / `run` / `debug` サブコマンドに `--error-format <json|human>` を追加。デフォルトは `human`。

```
sprs run --error-format json main.sprs
sprs build --error-format human
```

`main.rs` の引数解析（現状 `--dest` 処理と同様のループ）に追加。

### 8.2 出力先

- **human**: stderr（現状通り）。`eprintln!`
- **json**: stdout。1エラー = 1 JSON オブジェクト。複数エラーは newline-delimited JSON (NDJSON)

```json
{"code":"SPRS-SYN-002","category":"Syntax",...}
{"code":"SPRS-SEM-002","category":"Semantic",...}
```

---

## 9. 非目標 (Non-Goals)

- **言語内ドキュメント参照機構の実装**: `doc_ref` フィールドは URL 文字列のみ定義。実際に `sprs://errors/SPRS-SEM-003` でドキュメントを引く機構は別spec。
- **実行時 panic の context 出力**: Phase 3b で issue #26 解決後に対応。
- **実行時 panic の location 出力**: Phase 3a。Phase 2 完了後に `__panic` ABI 拡張で着手可能だが、本 spec では対象外。
- **既存 `format!` ベースエラーメッセージの国際化**: help 文は日本語でハードコード。i18n は別spec。

---

## 10. テスト戦略

### 10.1 Phase 1 テスト

- `error_helper`（→ `error_reporter`）の `ParseError` → `SprsError::Parse` 変換の単体テスト
- `render` 関数の human/json 両モード出力のスナップショットテスト
- 既存 parse error を発生させるテストケース（不正トークン、EOF 等）で JSON 出力を検証

### 10.2 Phase 2 テスト

- `Spanned<T>` の生成が正しい span を持つことのテスト
- 各エラーコード（SPRS-SEM-001 〜 012, SPRS-TYP-001 〜 004）の発生と出力を検証
- 全テストスイート（87 PASS / 7 XFAIL / 8 FAIL）が回帰しないこと

---

## 11. 将来拡張（参考）

### Phase 3a: 実行時 panic の location

`__panic` の ABI を拡張し、file/line/col を静的引数として埋め込む。

```rust
// 現状
extern "C" fn __panic(message_ptr: *const i8)

// Phase 3a
extern "C" fn __panic(message_ptr: *const i8, file_ptr: *const i8, line: u32, col: u32)
```

`create_panic_err` が呼ばれる codegen サイトで、対応する AST ノードの `Span` から file/line/col を取得して IR に埋め込む。LLVM debug metadata は不要。

### Phase 3b: 実行時 panic の context

値を読んでフォーマットする IR を各 panic サイトで生成。`Tag::Error` で回復可能なエラーにする（issue #26 依存）。

---

## 12. オープン質問

なし。全ての設計判断はユーザー承認済み:

| 項目 | 決定 |
|---|---|
| 方式 | 案3: 構造化エラーレポート |
| 対象フェーズ | Phase 1 (parse) + Phase 2 (意味/型) |
| 実行時 panic | 対象外（Phase 3a/3b は将来拡張） |
| 出力モード | human (default) + json (`--error-format=json`) |
| エラーコード | `SPRS-<CAT>-<NNN>` |
| span 表現 | `Span { start: usize, end: usize }` |
| span thread 手法 | `Spanned<T>` ラッパー |
| エラー型 | `SprsError` enum 新設 |

---

*Spec author: sprs design session*
*Date: 2026-07-27*

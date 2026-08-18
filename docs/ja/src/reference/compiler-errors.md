# コンパイルエラー

この章では **コンパイル時** の診断を説明します。
ランタイムのエラーラベル（`{:error, ...}`、`?`、未捕捉の `main`）は [エラー](../language/errors.md) にあります。
CLI フラグは [入門](../getting-started.md) にあります。

## コード

コードは 3 桁の番号付き `SPRS-<SYN|TYP|SEM>-NNN` です。
内部失敗は JSON `code` `SPRS-INTERNAL` を使います。
human の内部出力は `internal error: ...` です（`Display` 形は `Internal error: ...`）。

## 形式の選択

`sprs build|run|debug` の `--error-format` は `sprs.toml` の `error_format` より優先されます。
どちらも設定されていなければ、形式は `human` です。
許可される値: `human`、`json`、`json-pretty`。
JSON と `json-pretty` の報告は stdout へ、`human` の報告は stderr へ出ます。
コンパイル失敗時の終了ステータスは `1` です。

## JSON スキーマ

すべての JSON オブジェクトは次のキーを使います。

| Key | Meaning |
|-----|---------|
| `code` | `SPRS-SYN-001` 形式、または `SPRS-INTERNAL` |
| `category` | `Syntax`、`Semantic`、`Type`、または `Internal` |
| `phase` | 常に `"compile"` |
| `severity` | 常に `"error"` |
| `message` | 診断テキスト |
| `location` | オブジェクト: `file`、`line`、`column`、`end_line`、`end_column`、`snippet` |
| `expected` | 一部の構文解析エラーではトークン名。それ以外は `[]` |
| `expected_type` | 一部の型エラーに存在する |
| `actual_type` | 一部の型エラーに存在する |
| `help` | 任意のヘルプ文字列 |

行番号と列番号は 1 始まりです。
位置のない Internal エラーは `file` `"<unknown>"` と行/列 `0` を使います。

## Human 形式

human 報告は `error[CODE]`、`--> file:line:column` 位置、ソース断片、存在する場合は `expected` / `help` を使います。
Internal 報告は `error[CODE]` の代わりに `internal error:` を使います。

## コード一覧

同じ番号が複数のメッセージを覆うことがあります。
表はパターンを列挙します。
番号は単一の意味ではありません。

### 構文（`SPRS-SYN-…`）

| Code | Message patterns |
|------|------------------|
| SYN-001 | `InvalidToken` |
| SYN-002 | `UnrecognizedToken '{token}'`; `` `{keyword}` is a reserved keyword `` (identifier position; help: `use ^{keyword} if this name is intentional`) |
| SYN-003 | `ExtraToken '{token}'` |
| SYN-004 | `UnrecognizedEOF` |
| SYN-005 | `Invalid assignment target` を含むユーザー構文解析エラー（メッセージはパーサ文字列） |
| SYN-006 | `Expected IDENT token`、`Expected MACRO token`、`Expected NUM token`、`Expected FLOAT token`、または `Expected StrLiteral token` を含むユーザー構文解析エラー。`invalid FunctionBuild directive @{name}`。SYN-005 でも SYN-007 でもない他の `ParseError::User` メッセージも含む |
| SYN-007 | （`@init` 用としては未使用。構造体初期化はコア構文 `init Type { ... }`） |
| SYN-008 | `unnecessary identifier escape \`^{name}\``（help: `use {name} instead of ^{name}`） |

### 型（`SPRS-TYP-…`）

| Code | Message patterns |
|------|------------------|
| TYP-001 | `Type mismatch: Function expects pointer type (e.g. str) but got {type} from expression {expr}` |
| TYP-002 | `Type mismatch: Function expects Bool but got {type} from expression {expr}` |
| TYP-003 | `Type mismatch: Function expects Int type but got {type} from expression {expr}` |
| TYP-004 | `Type mismatch: Function expects Float type but got {type} from expression {expr}` |
| TYP-005 | `Type mismatch: Function declares >> {expected} but return expression has {actual}` |
| TYP-006 | `Type mismatch: cannot assign {rhs} to fixed binding `{name}` of type {ty}` |
| TYP-007 | `Type mismatch: argument {n} of `{fn}` expects {ty}, found {actual}`；`Type mismatch in call to `{fn}`: type parameter `{T}` was already resolved to `{ty}`, but the argument has type `{actual}`；`Type mismatch in call to `{fn}`: multiple `when` rules matched`；未解決の型パラメータ |

### 意味（`SPRS-SEM-…`）

現行コンパイラに `SEM-001`、`SEM-012`、`SEM-014` はありません。

| Code | Message patterns |
|------|------------------|
| SEM-002 | `Undefined variable: {name}`；`Undefined variable in dynamic label name: {name}`；`attach slot '<:{name}' used before @attach` |
| SEM-003 | `Unknown macro: {name}`；`@is_error` / `@error_message` / `@label_payload` / `@label_name` は引数ちょうど 1 つを期待する；`@error expects exactly 1 argument: reason`；`@attach expects exactly 2 arguments: expression and label`；`@attach second argument must be a slot such as <:name`；`@label_is expects exactly 2 arguments: value and label`；`@label_is second argument must be an atom such as :name or :"{i}-item"`；`dynamic label name part `{part}` has type {ty}; only int/bool/str allowed` |
| SEM-004 | `Undefined closed label member: {set.member}`；`Duplicate closed label set: {name}`；`Duplicate label: {name}` |
| SEM-005 | （削除。旧 `@init` は `Unknown macro: init` / SEM-003） |
| SEM-006 | `Unknown runtime function: {name}` |
| SEM-007 | `Field '{field}' not found in struct '{name}'`；`Undefined struct : {name}` |
| SEM-008 | `@cast second argument must be a type identifier : {expr}` |
| SEM-009 | `Unsupported target type for @cast: {ty}` |
| SEM-010 | `Failed to read module file {path}: {error}` |
| SEM-011 | `Undefined type: {name}`；`` `Self` is only valid in struct field type annotations ``；`unknown type `{legacy}`; use {replacement}`（`int`→`i64`、`list`→`List(T)`、`err`→`Label(:error, Any)`、`atom`/`label`→`Label` など）；`List requires exactly one type argument`；`Label application must be Label or Label(:name, T)` |
| SEM-013 | マクロ引数個数（`list_push expects 2 arguments`、`buf_len expects 1 argument`、`buf_get expects 2 arguments`、`buf_set expects 3 arguments`、`@clone expects 1 argument`、`@move expects 1 argument`、`@move expects a variable argument`、`@cast expects 2 arguments`、`@fcast expects exactly 1 argument`、`@lshift expects 2 arguments (value, shift_amount)`、`@rshift expects 2 arguments (value, shift_amount)`、`@not expects 1 argument`）；`@raw` / `@free` は unsafe ブロックを要求する；`Undefined variable: {name}`；`Module '{name}' not found`；`Function '{fn}' not found in module '{module}'`；`Undefined struct : {name}`；`Field '{field}' not found in struct '{name}'`；`Field definition for '{field}' not found in struct '{name}'`；`unknown field `{field}` in init {Type}`；`duplicate field `{field}` in init {Type}`；`missing required field `{field}` in init {Type}` |
| SEM-015 | `Undefined function: {name}`；`` `@raw` requires an unsafe block ``；`` `@free` requires an unsafe block `` |
| SEM-016 | `Argument count mismatch: function `{fn}` expects {n} argument(s), found {m}` |
| SEM-017 | `match patterns must be static :name in v1`；`payload pattern requires Label scrutinee`；`case _ must be the last match arm`；`non-exhaustive match on {set}; missing {Set.member, ...}` |
| SEM-018 | ``undefined FunctionBuild `{name}` `` |
| SEM-019 | ``duplicate FunctionBuild `{name}` `` |
| SEM-020 | `duplicate FunctionBuild directive {name}`（`params` / `return_type` / `visibility`） |
| SEM-022 | ``FunctionBuild `{name}` is private and cannot be used from an external source`` |
| SEM-023 | ``multiple `function_build source` directives in one file`` |
| SEM-025 | `function names must use snake_case`（他分類: モジュール/変数/フィールド/マクロは snake_case、型名は PascalCase、ラベルメンバーは snake_case） |
| SEM-024 | `circular FunctionBuild source: {a} -> {b} -> ...` |

### Internal

| Code | Message patterns |
|------|------------------|
| SPRS-INTERNAL | コンパイラのバグと残った `From<String>` 変換（`Internal error: ...` / JSON `SPRS-INTERNAL`） |

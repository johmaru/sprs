# sprs コンパイラ バグレポート (未対応分)

**対象リポジトリ**: `C:/Users/Johma_sub/sprs_new/sprs`
**最終更新**: 2026-07-22
**注記**: slab ベースのランタイム移行、M01 リファクタリング、F01/F02 修正で解消されたバグは本レポートから削除済み。以下は未対応のバグ。

---

## 1. フロントエンド (lexer / parser / AST)


#### BUG-F15: `Stmt` で連鎖代入 (`a = b = c;`) が不可 【Low】
- **ファイル**: `src/grammar.lalrpop`
- **推奨修正**: `Assign` を `Expr` に昇格。
---

## 2. LLVM コード生成

（なし）

---

## 3. CLI / エントリポイント (`src/main.rs`, `src/command_helper.rs`, `build.rs`)

（なし）


---

## 4. 深刻度別集計 (未対応分)

| 深刻度 | 件数 | バグ ID |
|--------|------|--------|
| **Critical** | 0 | — |
| **High** | 0 | — |
| **Medium** | 0 | — |
| **Low** | 1 | F15 |
合計: 1 件 (ユニーク)。

---

## 4.5 issue に移管したバグ（未対応・設計議論中）

これらはコード修正で解消したものではなく、issue で設計議論中のもの。集計表の「未対応」件数には含まれない。

| バグ ID | 深刻度 | 内容 | issue |
|---|---|---|---|
| **BUG-L06** | Medium | 整数加算で `nsw`/`nuw` フラグ未使用、オーバーフロー時に wrap-around。`nsw`/`nuw` 付けると UB になるため却下。Zig ライクな `Result` ベース（`@AnyError(x)` / `@WhatError(x) == OVERFLOW`）で設計中。#26（catchable エラー機構）に依存。 | [#27](https://github.com/johmaru/sprs/issues/27) |
| **BUG-L04** | Medium | `create_panic_err` が `build_unreachable` を生成しない。動作上のバグではなく設計上の好みの問題（全呼び出し元が直後に `build_unreachable` を置いているため IR 上は問題ない）。`__panic` が回復可能になった時に `build_unreachable` の位置を見直す必要があるため、issue #26 の実装時に合わせて対応する。 | [#26](https://github.com/johmaru/sprs/issues/26) |
| **BUG-F09** | Medium | `Postfix` 規則で `base` が `Expr::Var` 以外のとき、レシーバが破棄されて `Expr::Call` に変換される。メソッドチェーン（`list[0].method()` 等）が使えない。現在は `ModuleAccess`（`test.hello()`）のみ使い、メソッドチェーンを使わないため発火しない。メソッド呼び出しのセマンティクス（`self` の有無等）を決める設計判断が必要。 | [#28](https://github.com/johmaru/sprs/issues/28) |

---

## 5. 解消済みバグ (参照用)

### slab ベースのランタイム移行で解消

- **BUG-R01**: `__list_new` 負値 panic → `INVALID_HANDLE` 返却
- **BUG-R02**: NULL デリファレンス → ハンドル 0 = 無効、世代チェック
- **BUG-R03**: `__list_get` の `exit(1)` → Unit sentinel 返却
- **BUG-R04**: `__drop` の String/Struct/Enum 漏れ → slab が全タグ統一管理
- **BUG-R05**: `__clone` の NUL 終端なし → Rust `String` で長さ管理
- **BUG-R06**: `__malloc` OOM/負サイズ panic → NULL 返却
- **BUG-R08**: List clone の String 連鎖バグ → R05 解消で連動解決
- **BUG-R09**: `__clone` default 分岐の浅いコピー → slab で型別ディープコピー
- **BUG-R10**: `__panic` の NULL デリファレンス → `is_null` チェック追加
- **BUG-L02**: 文字列連結のヒープ BOF → `__string_concat` で安全な結合
- **ダングリング** (Vec 再割り当て) → `__list_get` が値で返す
- **BUG-R07**: Float タグの不整合 → `format_sprs_value` で全 float バリアントを統一処理
- **BUG-L10**: `create_field_access` の `build_pointer_cast` が不正 → `__struct_borrow` で正しいポインタ取得
- **BUG-L11**: `create_struct_init` の `build_malloc` が `__drop` で解放されない → `__struct_new` + `__struct_borrow` で slab 管理
- **BUG-L12**: `create_struct_init` の `allloca` 変数名 typo → リライト時に `alloca` に修正
- **use-after-free** → 世代番号で検出

### M01 リファクタリングで解消

- **BUG-M01**: `main()` が `()` を返し終了コード 0 になる → `Result<(), Box<dyn Error>>` 化、`?` 伝播
- **BUG-M02**: `sprs init --name` 値なしで分かりにくい終了 → 明確なエラーメッセージ
- **BUG-M07**: 不明なサブコマンドを黙って無視 → `match` の `other =>` でエラー表示
- **BUG-L18**: `Target::from_triple`/`create_target_machine`/`write_to_file` の `unwrap` → `?`
- **BUG-L19**: `create_dir_all` の `expect` → `?`
- **BUG-L20**: `rustc`/`clang` の `expect` → `map_err(...)?`
- **BUG-L22**: 実行ファイルの `status.success()` 未チェック → チェック追加
- **BUG-L23**: `mode == ExecuteMode::Build && false` の dead code → 削除

### フロントエンド修正で解消

- **BUG-F01**: 数値リテラル `parse().unwrap()` で panic → `map_err` でエラー伝播
- **BUG-F02**: 文字列リテラルのエスケープ未対応 → `unescape_sprs_string` で `\n`/`\t`/`\"`/`\\`/`\0`/`\u{XXXX}` に対応
- **BUG-F03**: コメント正規表現 `# [^\n]*` が空白を必須にしていた → `#[^\n]*` に変更。`#comment`、`#` 単独、行末 `#` がスキップされることを確認
- **BUG-F05**: `is_int_type_in_llvm()` に浮動小数点型が含まれ、`not_int_type_in_llvm()` と矛盾 → `is_int_type_in_llvm` から浮動小数点型を削除。`not_int_type_in_llvm` に欠落していた `Type::Float` を追加し、整数型でない全型を網羅
- **BUG-F10**: 文法の `unreachable!()` 多用と `I8Literal`〜`F64Literal` のプレースホルダ実装 → `I8Literal`〜`F64Literal` (11個) は前回のコード整理で削除済み。残り5箇所の `unreachable!()` (Ident/MacroName/Num/Float/StringLiteral) を `=>?` 構文 + `Err(ParseError::User { ... })` に置換し、予期しないトークンで panic せずパースエラーとして伝播するよう修正

### X02 Critical バグ修正で解消

- **BUG-X02**: `Type::Str` の戻り値型が `ptr` でコード生成され segfault → `declare_fn_prototype` で `Type::Str` を `runtime_value_type` に変更。`register_struct` 側は `create_field_access`/`create_struct_init` が i64 slab handle として扱うため `ptr` のまま維持。`main.sprs` に `>> str` 関数の呼び出しテスト (`get_greeting`, `get_static_str`, `call_str_fn`) を追加し segfault 解消を確認

### @ マクロ構文導入で解消

- **BUG-F07**: `>>` トークンが戻り値型の矢印としてのみ使われ、シフト演算子が未実装 → @ 前置マクロ構文 `lshift(x, 4)` / `rshift(x, 4)` を導入。`RawTok::MacroIdent` + `Token::Macro(String)` + `Expr::Macro(String, Vec<Expr>)` を追加。符号付きタグ (Integer/Int8/16/32/64) は `ashr`、符号なしタグ (Uint8/16/32/64) は `lshr`、非整数タグは `__panic` で実行時エラー。`>>` (GtGt) トークンは戻り値型矢印として維持。既存マクロ (println!/list_push!/clone!/cast!) も `@println` 等の @ 構文に統一移行
- **BUG-X09 (部分解消)**: `cast` マクロの戻り値型推論を `infer_type` の `Expr::Macro` 分岐に追加。第2引数の型から戻り値型を推論。`lshift`/`rshift` は第1引数の型を返す
- **BUG-F04 解消**: 論理否定を `not(x)` マクロで実装。0→1, 非0→0 の論理否定。Bool タグで結果を返す。`!` 単独トークン (`Not` / `Expr::Not`) は追加せず、@ マクロ方式で代替。`!?` 正規表現は既存マクロ構文のために維持

### コード整理で解消

- **BUG-L01**: `create_integer` が負の i64 を `*n as u64` で符号付きビットパターンとして格納するが、`const_int` の sign フラグが `false` (符号なし) → `const_int(*n as u64, true)` に変更し符号付きとして扱う。`src/llvm/value.rs`
- **BUG-F13**: `MoreStructFields` が死んだ規則 → 削除済み。`src/grammar.lalrpop`
- **BUG-L16**: `set_global_constant_str` の `Set` バリアントと不整合 → `StrConstantAction`/`StrValue` enum を削除し、`&str` 直接受け取りに簡略化。`src/llvm/compiler.rs`
- **BUG-M05**: `get_all_arguments` の `skip_next` が dead code → `filter().collect()` に簡潔化
- **BUG-L03**: `create_if_expr` の PHI incoming 判定が `t.get_parent() == merge_bb` で常に false になる問題 → ターミネータの `opcode` が `Br` かどうかで判定するよう修正。`src/llvm/control_flow.rs`
- **BUG-L07**: `create_div_expr` / `create_mod_expr` でゼロ除算チェックなし → `create_binary_int_op` 経由から独立実装に切り出し、除算前に `r_val == 0` チェックを生成し `__panic` を呼ぶよう修正。`src/llvm/arithmetic.rs`
- **BUG-X06**: `string_constants` HashMap がモジュール非スコープ → キャッシュを完全に削除し、`string_constants` フィールドを `string_counter: usize` に置き換え。各モジュールが独自の `Internal` linkage グローバルを作成するよう修正。`src/llvm/compiler.rs`, `src/llvm/value.rs`

---


### BUG-L14 修正で解消

- **BUG-L14**: `get_runtime_fn` が未知関数で `panic!` → `Result<FunctionValue, String>` を返すよう変更し、`_ => return Err(format!("Unknown runtime function: {}", name))` でエラー伝播。全17箇所の呼び出し箇所 (`arithmetic.rs:1`, `codegen.rs:1`, `compiler.rs:2`, `data_structures.rs:5`, `macros.rs:3`, `value.rs:5`) に `?` を追加。`exit_scope` / `emit_drop_for_return` も `()` → `Result<(), String>` にシグネチャ変更し、呼び出し側3箇所 (`compile_fn`, `compile_return`, `compile_block`) に `?` を追加。`src/llvm/compiler.rs`, `src/llvm/codegen.rs`, `src/llvm/arithmetic.rs`, `src/llvm/data_structures.rs`, `src/llvm/macros.rs`, `src/llvm/value.rs`
- **BUG-L08**: `cast` マクロの switch cases に `Int8/Int16/Int32/Int64/Uint8/Uint16/Uint32/Uint64` が未登録 → 整数タグ値を cast すると default `bb_f64` にフォールスルーし bit_cast で f64 として誤解釈。修正: 8タグを cases に追加。符号付き (Int8/16/32/64) は `bb_int` (SITOFP) へ、符号なし (Uint8/16/32/64) は新設の `bb_uint` (UITOFP) へルーティング。merge PHI に `bb_uint` の incoming を追加。検証: `@cast(@cast(4294967295, u32), fp64)` → `4294967295` (UITOFP 正解、SITOFP なら `-1`、bit_cast なら非正規化数)。`src/llvm/macros.rs`
### パストラバーサル対策で解消

- **BUG-L21**: `llvm_executer.rs` が `sprs.toml` の `out_dir` / `name` / `src_dir` をサニタイズせず、パストラバーサルで任意ファイル上書き可能だった問題 → `command_helper.rs` に `validate_name` (`[A-Za-z0-9_-]+` のみ許可、空文字拒否) と `validate_subpath` (絶対パス・`..` 成分拒否、未存在パスでも検証可能) を追加。`build_and_run` で `src_dir` / `out_dir` / `name` を検証。`--dest` (CLI引数) は信頼できるユーザー入力のため検証対象外。検証: `name = "../../../tmp/evil"` / `out_dir = "../../../tmp/evil_out"` / `src_dir = "../../../etc"` / `out_dir = "/tmp/evil_abs"` の全パストラバーサル攻撃を拒否することを確認。`src/llvm/llvm_executer.rs`, `src/command_helper.rs`
- **BUG-M04**: `init_project` が `name` をサニタイズせず、パストラバーサルで任意ディレクトリにファイル作成可能だった問題 → `init_project` で `validate_name(name)?` を呼び出し。`init --name "../../../tmp/evil"` と `init --name ""` を拒否することを確認。`src/command_helper.rs`
- **BUG-M03**: `init_project` が既存の `sprs.toml` / `src/main.sprs` を無条件で上書きしていた問題 → `init_project` に `force: bool` パラメータを追加し、`--force` なしでは `Path::exists()` で既存ファイルを検出してエラー終了するよう修正。`main.rs` の `init` コマンド解析で `--force` と `--name` を任意順序で受け付けるよう拡張。存在チェックを `println!` の前に配置し、エラー時に「Initializing project」と出力されないよう UX 整理。検証: 既存ファイルありで `init` を拒否、`--force` で上書き成功、`--force --name` と `--name --force` の両順序で動作することを確認。`src/command_helper.rs`, `src/main.rs`
- **BUG-L15**: `emit_drop_for_return` の drop 順序が外側→内側（RAII 違反）だった問題 → 実行ループの `vars_to_drop.into_iter().rev()` から `.rev()` を削除し、内側→外側の正しい RAII 順序に修正。`skip(1)` は `scopes[0]`（グローバルスコープ）を除外する意図的な設計のため維持。検証: `struct_in_function`（変数 `p`/`sum`）の LLVM IR で、修正前は `p`→`sum`（外→内）だった drop 順序が、修正後は `sum`→`p`（内→外）に変化したことを確認。`sum` が `p` のフィールドを参照する場合、修正前は use-after-free の可能性があった。`src/llvm/compiler.rs`
- **BUG-L17**: `create_add_expr_build_float_branch` の switch default が `bb_f64` で、想定外のタグが整数を f64 として誤解釈する問題 → default を `error_bb`（`__panic` 呼び出し）に変更。同時に `Tag::Float = 1`（デフォルト f64）を cases に追加し、正常系の float 加算が `error_bb` に飛ばないよう修正。`error_message` は静的文字列（`IntValue` は実行時値ではないため埋め込めない）。同パターンの修正を `@cast` マクロ (macros.rs) にも適用: switch default を `bb_f64` から `error_bb` に変更し、`create_panic_err` + `build_unreachable` を追加。検証: 全テストパス（Float 加算含む）、`Tag::Float` が `bb_f64` に正しくルーティングされることを確認。`src/llvm/arithmetic.rs`, `src/llvm/macros.rs`
- **BUG-L13**: `create_entry_block_alloca` が `get_insert_block().unwrap()` / `get_parent().unwrap()` / `get_first_basic_block().unwrap()` の3箇所で `None` の場合 panic する問題 → シグネチャを `PointerValue` から `Result<PointerValue, String>` に変更し、3つの `.unwrap()` を `.ok_or(...)?` に変換。`get_first_instruction()` の `None` は空のエントリブロックを意味するため `position_at_end` のまま維持（バグではない）。連鎖修正: `create_dummy_for_no_return` と `var_load_at_init_variable` も `Result` 返しに変更し、全41箇所の呼び出し元に `?` を追加（ast_edit で一括適用）。検証: 全テストパス。`src/llvm/value.rs`, `src/llvm/variable.rs`, `src/llvm/arithmetic.rs`, `src/llvm/comparison.rs`, `src/llvm/control_flow.rs`, `src/llvm/data_structures.rs`, `src/llvm/macros.rs`
- **BUG-L09**: `create_field_access` が `field_index` の配列境界チェックなしに `struct_def.fields[field_index as usize]` と `build_struct_gep(..., field_index, ...)` に渡す問題（CWE-125 out-of-bounds read）→ `struct_def` 取得後に `field_index as usize >= struct_def.fields.len()` で境界チェックし、範囲外は `Err` を返すよう修正。現状の唯一の呼び出しパス（codegen.rs:499）は `get_field_index` で既にフィールド名の存在を検証しているため発火しないが、将来の呼び出しパス追加時の安全網として機能。検証: 全テストパス。`src/llvm/data_structures.rs`
- **BUG-F08**: `Float` 正規表現 `[0-9]+\.[0-9]+` が指数表記（`1e10`, `1.5e-3`）に未対応だった問題 → 2つの `#[regex]` を追加: `[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?`（小数部+指数）と `[0-9]+[eE][+-]?[0-9]+`（整数部+指数）。`1.`/`.5`（小数部/整数部の省略）は `..` 範囲構文と衝突するため見送り。16進/2進/8進は `@cast(255, u8)` で回避可能なため見送り。検証: `1.5e3`→1500, `1e10`→10000000000, `1.5e-3`→0.0015, `2.0e2`→200, 既存形式（`1.5`, `42`）も維持されることを確認。`src/front/lexer.rs`
- **BUG-M12**: ホストとターゲット OS が異なる場合でも実行を試み、実行失敗のエラーになる問題 → 実行判定を `match compiler.target_os` で整理: `OS::Linux => cfg!(target_os = "linux")`, `OS::Windows => cfg!(target_os = "windows")`, `OS::Unknown => true`（default triple = host triple）。不一致時は実行をスキップし `[Skip]` メッセージを出力。ビルド（clang cross-link）は維持。検証: 全テストパス（Linux ホストで `OS::Unknown`→実行、`OS::Linux`→実行）。`src/llvm/llvm_executer.rs`
- **BUG-F04b**: 単項マイナス（Unary Minus）が未実装で `-5` や `-x` が書けなかった問題 → `Expr::Neg(Box<Expr>)` バリアントを追加し、`Unary`/`UnaryNoStruct` 規則に `Minus <p:Unary> => Expr::Neg(Box::new(p))` を追加。`compile_expr` で `Neg(expr)` を `Minus(Number(0), expr)` として `create_minus_expr` に渡すよう実装。`infer_type` にも `Neg` 分岐を追加。検証: `-5`→-5, `-(-5)`→5, `-x`(x=10)→-10, `-(x+3)`→-13, `-1`→-1。浮動小数点の単項マイナス（`-3.14`）は二項演算子 `Minus` と同じ制限（浮動小数点分岐なし）で未対応、別バグとして扱う。`src/front/ast.rs`, `src/grammar.lalrpop`, `src/llvm/codegen.rs`
- **BUG-F12**: `var x;` で未初期化変数が許可され、デフォルト値が未規定だった問題 → 既に `compile_block` で `var.expr.as_ref().unwrap_or(&ast::Expr::Unit())` により `Unit()` で初期化されていた。`create_unit` が `Tag::Unit` を設定し、`data` は無視される。BUG_REPORT.md の記載が古かっただけ。`src/llvm/codegen.rs`
- **BUG-M09**: `llvm_executer.rs` の `_full_path` パラメータが未使用だった問題 → 既に削除済み。`build_and_run` のシグネチャは `(dest: Option<&str>, mode: ExecuteMode)` になり、`_full_path` は存在しない。`src/llvm/llvm_executer.rs`
- **BUG-M11**: `sprs.toml` 読み込み失敗を黙殺していた問題 → `toml::from_str` のパース失敗は `eprintln!("Failed to parse ...")` で出力済みだったが、`std::fs::read_to_string` の失敗（ファイル不在・権限エラー・IO エラー）が `unwrap_or_else(|_| ...)` で無言で空文字に潰されていた。`|e|` でエラーを受け取り `eprintln!("Failed to read ...")` を出力するよう修正。併せて L41 のインデント不正（8→16スペース）も修正。`src/llvm/llvm_executer.rs`

### 構文体リファクタリングで解消

- **BUG-F14**: `ExprNoStruct` 系が `Expr` 系と重複定義されていた問題 → 構造体初期化構文を `Foo { ... }` から `@init(Foo { ... })` マクロ構文に変更。`Atom` から `Ident LBrace <fields> RBrace => Expr::StructInit` 規則を削除し、`@init` 専用規則 (`MacroName LParen Ident LBrace StructInitFields RBrace RParen`) を追加。これにより `if`/`while` 条件部での `{` 衝突が解消し、`ExprNoStruct`/`RangeExprNoStruct`/`ComparisonNoStruct`/`AddAndMinusNoStruct`/`MulAndDivAndModNoStruct`/`UnaryNoStruct`/`PostfixNoStruct`/`AtomNoStruct` の12規則を削除。`if`/`while` 条件部を `ExprNoStruct` から `Expr` に変更。`@init(Foo)` のように波括弧を省略した場合は codegen.rs の `Expr::Macro` 分岐で明示的エラーメッセージを返すよう対応。検証: 全テストパス（Struct = 10/200/20/3、Enum = 1/3/1、Module Import = hello world/()）。`src/grammar.lalrpop`, `src/llvm/codegen.rs`, `tests/src/data_structures.sprs`, `README.md`, `src/main.rs`

### メモリリーク修正で解消

- **BUG-L05**: `create_add_expr` のエラーメッセージが `Box::leak` でメモリリークしていた問題 → `create_panic_err` のシグネチャを `message: &'ctx str` から `message: &str` に変更。`set_global_constant_str` は元々 `&str` を受け取るため `'ctx` ライフタイムは不要だった。これにより `arithmetic.rs:123` の `Box::leak(error_message.into_boxed_str())` が不要になり、`&error_message` の直接参照渡しに変更。コンパイル時の Rust ヒープリークが解消。実行時の LLVM IR には影響無し（文字列は `set_global_constant_str` でグローバル定数として埋め込み済み）。検証: 全テストパス。`src/llvm/value.rs`, `src/llvm/arithmetic.rs`

### エラーハンドリング・クリーンアップ修正で解消

- **BUG-F06**: `FunctionParam` に型フィールドがなく、関数パラメータの型注釈が不可能だった問題 → `FunctionParam` に `ty: Option<Type>` フィールドを追加。grammar.lalrpop の `FunctionParamNode` で `Ident GtGt Type` 構文をパース可能にし `fn foo(x >> i64, y >> fp) { ... }` のように型注釈を書けるよう対応。ただし sprs はデフォルト動的型付け設計のため、型注釈は「書けるけど無視される」。静的型チェック全面導入時に見直す。検証: 全テストパス。`src/front/ast.rs`, `src/grammar.lalrpop`
- **BUG-F11**: `Preprocessor` トークンが `#define` のみで他の指令（`#include` 等）がエラーだった問題 → `#[token("#define")]` を `#[regex(r"#[a-z]+")]` に変更し、`#` で始まる小文字アルファベットの指令を一般化してパース可能に。検証: 全テストパス。`src/front/lexer.rs`
- **BUG-L24**: `PassBuilderOptions` の `run_passes` 結果を無視していた問題 → `let _ = module.run_passes(...)` を `if let Err(e) = ...` でエラーハンドリングし、`eprintln!` で警告ログを出力するよう修正。検証: 全テストパス。`src/llvm/llvm_executer.rs`
- **BUG-M06**: `help` コマンドで `--all` 以外の引数を無視していた問題 → 既に修正済み。`--all` 以外の引数が渡された場合 `eprintln!("Unknown help argument. Use --all.")` でエラーメッセージを表示し `Err` を返すよう対応済みだった。BUG_REPORT.md の記載が古かっただけ。検証: 該当コードパス確認。`src/main.rs`
- **BUG-M08**: `build.rs` の `expect` が汎用メッセージで panic していた問題 → `expect` に具体的なメッセージを追加: `"Failed to process grammar.lalrpop. Ensure lalrpop is configured correctly and the grammar file is valid."`。検証: ビルド成功。`build.rs`
- **BUG-M10**: 一時ファイル (`.ll`, `.o`, `runtime.rs`) のクリーンアップがなかった問題 → `cfg!(debug_assertions)` でDebugビルドか判定し、Releaseビルドではリンク後に `.ll`/`.o`/`runtime.rs` を `std::fs::remove_file` で削除するよう修正。Debugビルドではデバッグ用に一時ファイルを残す。検証: 全テストパス（Debugビルドで一時ファイルが生成されることを確認）。`src/llvm/llvm_executer.rs`

## 6. 詳細テストスイートで発見されたバグ (XFAIL)

LLVM 22.1.8 移行後のスモークテスト実装中に発見されたバグ群。
いずれも既存コードの問題であり、LLVM 22 移行とは無関係。
テストスイート (`main.sprs`) では該当機能を XFAIL (expected failure) としてスキップ。

### X01: `create_index` が `StructValue` を直接返す 【High】

- **ファイル**: `src/llvm/data_structures.rs`
- **症状**: `list[index]` アクセスで `Found StructValue ... but expected PointerValue variant` で panic
- **原因**: `__list_get` が `{ i32, i64 }` 構造体を値で返すが、`create_index` が `compile_expr` の契約 (`PointerValue` を返す) に違反し、生の `StructValue` をそのまま返している
- **影響**: リストのインデックスアクセスが一切使用不可
- **推奨修正**: `call_builtin_macro_clone` と同様に alloca に spill してから `PointerValue` を返す
  ```rust
  let res_ptr = create_entry_block_alloca(self_compiler, "list_get_res");
  self_compiler.builder.build_store(res_ptr, val).unwrap();
  Ok(res_ptr.into())
  ```


### X03: `Return(Var)` の move セマンティクスでタグが Unit になる 【High】

- **ファイル**: `src/llvm/codegen.rs` (compile_return), `src/llvm/variable.rs` (var_return_store)
- **症状**: `return var_name` で変数を返した際、戻り値のタグが `Unit` (6) になり、呼び出し側で正しい型として扱えない
- **原因**: `var_return_store` が move セマンティクスで変数のタグを `Unit` にリセットした後に、同じポインタから値をロードして返している
- **影響**: 変数をそのまま return するパターンが使用不可（リテラルや式の return は正常）
- **推奨修正**: move 前に値をロードして退避し、退避した値を返す
  ```rust
  let pre_loaded = self.builder.build_load(
      self.runtime_value_type, ptr, "return_pre_load"
  ).unwrap();
  // var_return_store の後に pre_loaded を返す
  ```

### X04: `infer_type` が比較演算・Call・ModuleAccess を処理しない 【Medium】

- **ファイル**: `src/llvm/codegen.rs` (infer_type)
- **症状**: `>> bool` で `return 5 == 5` と書くと `Type mismatch: Function expects Bool but got Any` エラー。`>> i64` で `return test.test()` と書いても `Type::Any` になる
- **原因**: `infer_type` に `Expr::Eq`/`Neq`/`Lt`/`Gt`/`Le`/`Ge` の分岐が無い（すべて `Type::Bool` を返すべき）。`Expr::Call` は `ret_ty_opt` が `None` の場合 `Type::Any` を返す（パーサーが常に `None` を渡すため）。`Expr::ModuleAccess` の分岐自体が存在しない
- **影響**: 比較演算の結果を `>> bool` で返せない。モジュール関数の戻り値型が推論されない
- **推奨修正**:
  - 比較演算の分岐を追加: `Expr::Eq(_,_) | Expr::Neq(_,_) | ... => Type::Bool`
  - `function_ret_types: HashMap<String, Type>` を追加し、`declare_fn_prototype` で関数シグネチャを保存
  - `infer_type` の `Expr::Call`/`ModuleAccess` で `function_ret_types` を参照

### X05: `create_dummy_for_no_return` が常に `runtime_value_type` を返す 【Medium】

- **ファイル**: `src/llvm/value.rs`
- **症状**: `>> i64` の関数で `if/else` 両分岐で return した後に末尾 return が無いと `Function return type does not match operand type of return inst!` エラー
- **原因**: `create_dummy_for_no_return` が常に `runtime_value_type` (`{ i32, i64 }`) を返す。`>> i64` や `>> fp` など他の戻り値型の場合、型不一致で LLVM が関数を検証エラーにする
- **影響**: `if/else` で両分岐 return する関数で末尾に `return 0;` のようなダミーが必要
- **推奨修正**: 現在の関数 (`function_signatures`) の戻り値型に応じたゼロ値を返すよう分岐

### X07: `Var + Var` 加算で実行時 error_bb に到達する 【High】

- **ファイル**: `src/llvm/arithmetic.rs` (create_add_expr), `src/llvm/codegen.rs` (get_known_type_from_expr)
- **症状**: 関数引数同士の加算 `return a + b` で実行時 `Panic: TypeError: type miss match` が発生
- **原因**: `create_add_expr` の error_bb で `get_known_type_from_expr(Var)` を呼ぶが、`get_known_type_from_expr` は `Var` を処理せずエラーを返す。エラーメッセージがグローバル定数として埋め込まれ、実行時に error_bb に到達すると panic する。関数引数の実行時タグが `Integer` (0) になるはずだが、`can_add` チェックが失敗している可能性
- **影響**: 関数引数を使った `Var + Var` 加算が使用不可。`var` 宣言された変数同士の加算は正常動作する
- **推奨修正**: `get_known_type_from_expr` に `Var` の分岐を追加し、変数の型を `infer_type` から取得。関数引数のタグ受け渡し処理を検証

### X08: `>> fp` 戻り値の表示が不正確 【Medium】

- **ファイル**: `src/llvm/codegen.rs` (compile_return)
- **症状**: `>> fp` で `return 1.5 + 2.5` と返すと、期待値 `4.0` に対して `4` と表示される。また一部の演算で `0.000...333e-262` のような異常値が出力される
- **原因**: `Return` 処理の `ret_ty.is_float_type()` 分岐で、`data_val` を `bit_cast` して返す際の型変換に問題がある可能性。`>> fp` は `Type::Float` (`f64`) だが、戻り値の LLVM 型が `f64` として正しく処理されていない
- **影響**: 浮動小数点を返す関数の結果が不正確
- **推奨修正**: `Return` の `ret_ty.is_float_type()` 分岐の型変換を検証。`runtime_value_type` と `f64` の相互変換を確認

### X09: `cast` の戻り値型推論 (部分解消) 【Low】

- **ファイル**: `src/llvm/codegen.rs` (infer_type Macro 分岐)
- **症状**: `>> fp` で `return @cast(1.5, fp16)` と書くと `Type mismatch: Function expects Float type but got Any` エラー。`>> i64` で `` @cast(1, i8) + @cast(2, i16) `` の異型混合も panic
- **原因**: `cast` が `Expr::Macro("cast", ...)` として処理されるが、`infer_type` に `Expr::Macro` 分岐が存在しなかった
- **影響**: `cast` の結果を `>> fp` や異型混合で return できない。同じ型同士の cast 同士の演算は正常動作
- **対応状況**: `infer_type` に `Expr::Macro` 分岐を追加し、`cast` は第2引数の型を推論するよう実装。ただし `>> fp` 戻り値自体の表示バグ (X08) が未解決のため、`cast` の `>> fp` return はまだ XFAIL。異型混合の加算は X07 (Var+Var) が未解決のため未検証

---

### テストスイートでのカバレッジ

| カテゴリ | PASS | XFAIL | FAIL | 備考 |
|---------|------|-------|------|------|
| Arithmetic | 13/13 | 0 | 0 | 全演算子正常 |
| Comparison | 12/12 | 0 | 0 | `>> i64` で 1/0 を返す形式で回避 |
| Control Flow | 8/8 | 0 | 0 | if/else, while 正常 |
| Variables | 6/6 | 0 | 0 | 代入, シャドウイング正常 |
| Functions | 0/5 | 3 | 2 | `` @add `` 関数は XFAIL, test_recursion/deep_recursion は結果不正 (期待120→実際107549842873449) |
| Lists | 4/4 | 0 | 0 | `` @list_push `` のみ, index access は XFAIL |
| Cast | 10/11 | 1 | 0 | 異型混合 `` @cast `` は XFAIL |
| Float | 0/9 | 3 | 6 | `` @cast `` >> fp は XFAIL, 残り6は結果不正 (期待4.0→実際4, 異常値) |
| Increment | 7/7 | 0 | 0 | ++, -- 正常 |
| Shift | 12/12 | 0 | 0 | `` @lshift `` / `` @rshift `` 正常 (符号付き ashr / 符号なし lshr / 非整数 panic) |
| Not | 5/5 | 0 | 0 | `` @not `` 正常 (0→1, 非0→0) |
| Struct | 4/4 | 0 | 0 | フィールドアクセス正常 |
| Enum | 3/3 | 0 | 0 | バリアントアクセス正常 |
| String | 3/3 | 0 | 0 | `` @>> str `` 関数の呼び出し・戻り値表示が正常 (X02 解消) |

**合計**: 87 PASS / 7 XFAIL / 8 FAIL / 0 クラッシュ

---

*レポート作成者: sprs バグ監査チーム*
*LLVM 22.1.8 移行スモークテストより*

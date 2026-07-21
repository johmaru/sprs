# sprs コンパイラ バグレポート (未対応分)

**対象リポジトリ**: `C:/Users/Johma_sub/sprs_new/sprs`
**最終更新**: 2026-07-21
**注記**: slab ベースのランタイム移行、M01 リファクタリング、F01/F02 修正で解消されたバグは本レポートから削除済み。以下は未対応のバグ。

---

## 1. フロントエンド (lexer / parser / AST)

#### BUG-F03: コメント正規表現が `#` の直後の空白を必須にする 【High】
- **ファイル**: `src/front/lexer.rs:141`
- **症状**: `# comment` はスキップされるが、`#comment` や `#` 単独、行末の `#` は `Err` トークンになりパースエラー。
- **原因**:
  ```rust
  #[regex(r"# [^\n]*", logos::skip, allow_greedy = true)]
  Comment,
  ```
  `# ` の空白が必須。
- **推奨修正**: `#[regex(r"#[^\n]*", logos::skip)]`

#### BUG-F04: `Ident` の正規表現が末尾 `!?` を許可し、`!` 単独トークンが未定義 【Medium】
- **ファイル**: `src/front/lexer.rs:133`
- **症状**: `foo!` が識別子としてマッチする。`!` 単独 (`Not` / 論理否定) が `RawTok` に存在せず、`!flag` のような式が書けない。
- **推奨修正**: `!?` を削除し、`Not` トークンと `Expr::Not` を追加。

#### BUG-F05: `is_int_type_in_llvm()` に浮動小数点型が含まれ、`not_int_type_in_llvm()` と矛盾 【High】
- **ファイル**: `src/front/type_helper.rs:27-54`
- **症状**: `Type::Float`, `TypeF16/32/64` が `is_int_type_in_llvm()` と `not_int_type_in_llvm()` の**両方**に含まれる。
- **推奨修正**: `is_int_type_in_llvm` から浮動小数点を削除。

#### BUG-F06: `FunctionParam` に型フィールドがなく、関数パラメータの型注釈が不可能 【Medium】
- **ファイル**: `src/front/ast.rs:49-51`
- **推奨修正**: `ty: Option<Type>` フィールドを追加。

#### BUG-F07: `>>` トークンが戻り値型の矢印としてのみ使われ、シフト演算子が未実装 【Medium】
- **ファイル**: `src/front/lexer.rs:149`, `src/grammar.lalrpop:155,236`, `src/front/ast.rs:1-46`
- **推奨修正**: 戻り値型矢印を `->` に変更。

#### BUG-F08: `Num` / `Float` の正規表現が指数表記・16 進・`1.` 形式に未対応 【Medium】
- **ファイル**: `src/front/lexer.rs:135-138`
- **推奨修正**: Float に指数表記、Num に 16 進/2 進/8 進を追加。

#### BUG-F09: `ModuleAccess` で `base` が `Expr::Var` 以外の場合に破棄される 【Medium】
- **ファイル**: `src/grammar.lalrpop:453-470`
- **推奨修正**: `Expr::MethodCall(Box<Expr>, String, Vec<Expr>)` ノードを追加。

#### BUG-F10: 文法の `unreachable!()` 多用と `I8Literal`〜`F64Literal` のプレースホルダ実装 【High】
- **ファイル**: `src/grammar.lalrpop:373-515`
- **推奨修正**: `unreachable!()` を `Err(ParseError::User { ... })` に置換。

#### BUG-F11: `Preprocessor` トークンが `#define` のみで他の指令がエラー 【Low】
- **ファイル**: `src/front/lexer.rs:153-154`
- **推奨修正**: `#[regex(r"#[a-z]+")]` で一般化。

#### BUG-F12: `var x;` で未初期化変数が許可され、デフォルト値が未規定 【Low】
- **ファイル**: `src/grammar.lalrpop:383-386`, `src/front/ast.rs:74-78`
- **推奨修正**: Unit 型でゼロ初期化するか、型注釈必須にする。

#### BUG-F13: `MoreStructFields` が死んだ規則 【Low】
- **ファイル**: `src/grammar.lalrpop:172-179`
- **推奨修正**: 削除。

#### BUG-F14: `ExprNoStruct` 系が `Expr` 系と重複定義 【Low】
- **ファイル**: `src/grammar.lalrpop:520-604`
- **推奨修正**: LALRPOP の `precedence` 宣言に移行。

#### BUG-F15: `Stmt` で連鎖代入 (`a = b = c;`) が不可 【Low】
- **ファイル**: `src/grammar.lalrpop:327-340`
- **推奨修正**: `Assign` を `Expr` に昇格。

---

## 2. LLVM コード生成 (`src/llvm/compiler.rs`, `src/llvm/builder_helper.rs`)

#### BUG-L01: `create_integer` が負の i64 を `*n as u64` で符号付きビットパターンとして格納するが、tag を `Integer` とする意図が不明確 【High】
- **ファイル**: `src/llvm/builder_helper.rs:498-512`
- **推奨修正**: コメント追加または `const_int_signed` 系 API を検討。

#### BUG-L03: `create_if_expr` の PHI incoming 判定が `then_bb_end == merge_bb` と同値で、return を含む then ブロックで PHI が不正 【High】
- **ファイル**: `src/llvm/builder_helper.rs:3051-3062`
- **推奨修正**: `if then_bb_end.get_terminator().is_some() && then_bb_end != merge_bb` で判定。

#### BUG-L04: `create_panic_err` が `build_call` のみで `build_unreachable` を生成しない 【Medium】
- **ファイル**: `src/llvm/builder_helper.rs:166-172`
- **推奨修正**: `create_panic_err` の最後に `build_unreachable` を追加。

#### BUG-L05: `create_add_expr` のエラーメッセージが `Box::leak` でメモリリーク 【Low】
- **ファイル**: `src/llvm/builder_helper.rs:1138-1146`
- **推奨修正**: モジュールの global constant として登録。

#### BUG-L06: 整数加算全般で `nsw`/`nuw` フラグ未使用、オーバーフロー時に wrap-around 【Medium】
- **ファイル**: `src/llvm/builder_helper.rs:1517` (i64), `2296-2316` (i64_add_logic), 他多数
- **推奨修正**: 仕様に応じてオーバーフローチェックまたはフラグ追加。

#### BUG-L07: `create_div_expr` / `create_mod_expr` でゼロ除算チェックなし 【High】
- **ファイル**: `src/llvm/builder_helper.rs:2660, 2676`
- **推奨修正**: 除算前に `r_val == 0` のチェックを生成し `__panic` を呼ぶ。

#### BUG-L08: `cast!` マクロの switch cases に `Int8/Int16/Int32/Int64/Uint8/...` が未登録 → 整数を cast! すると f64 として誤解釈 【High】
- **ファイル**: `src/llvm/builder_helper.rs:3913-3918`
- **推奨修正**: Int8/Int16/.../Uint64 のケースを追加。

#### BUG-L09: `create_field_access` で `field_index` の境界チェックなし 【Medium】
- **ファイル**: `src/llvm/builder_helper.rs:3376`
- **推奨修正**: `field_index` を検証し、範囲外なら `Err` を返す。

#### BUG-L13: `create_entry_block_alloca` が `get_insert_block().unwrap()` で panic 【Medium】
- **ファイル**: `src/llvm/builder_helper.rs:184-198`
- **推奨修正**: `None` の場合は `Err` を返す。

#### BUG-L14: `get_runtime_fn` が未知関数で `panic!` 【High】
- **ファイル**: `src/llvm/builder_helper.rs`
- **推奨修正**: `Result` でエラー伝播。

#### BUG-L15: `compile_block` が early-return 時にスコープをリーク / `emit_drop_for_return` の drop 順序が逆 【High】
- **ファイル**: `src/llvm/compiler.rs`
- **推奨修正**: early-return 時もスコープ退出を保証。drop 順序を内側→外側に修正。

#### BUG-L16: `set_global_constant_str` のグローバル重複チェックが `Set` バリアントと不整合 / panic メッセージが `Box::leak` 【Medium】
- **ファイル**: `src/llvm/compiler.rs`
- **推奨修正**: 文字列プールで一元管理。

#### BUG-L17: `create_add_expr_build_float_branch` の switch default が `bb_f64` 【Medium】
- **ファイル**: `src/llvm/builder_helper.rs:1612, 1734-1738`
- **推奨修正**: default を `error_bb` に変更。

#### BUG-L21: `llvm_executer.rs` が `sprs.toml` の `out_dir` / `name` / `src_dir` をサニタイズせず、パストラバーサルで任意ファイル上書き 【High】
- **ファイル**: `src/llvm/llvm_executer.rs:42-57, 121-127, 172-180`
- **推奨修正**: `out_dir` / `name` をバリデーション。

#### BUG-L24: `PassBuilderOptions` の `run_passes` 結果を無視 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs:99-100`
- **推奨修正**: エラーをログ出力。

---

## 3. CLI / エントリポイント (`src/main.rs`, `src/command_helper.rs`, `build.rs`)

#### BUG-M03: `init_project` が既存の `sprs.toml` / `src/main.sprs` を無条件で上書き 【High】
- **ファイル**: `src/command_helper.rs:46-86`
- **推奨修正**: `Path::exists()` でチェックし `--force` フラグなしには上書きしない。

#### BUG-M04: `init_project` が `name` をサニタイズせず、パストラバーサルで任意ディレクトリにファイル作成の可能性 【High】
- **ファイル**: `src/command_helper.rs:31-88`
- **推奨修正**: `name` を `[A-Za-z0-9_-]+` にバリデーション。

#### BUG-M05: `get_all_arguments` の `skip_next` が dead code 【Low】
- **ファイル**: `src/command_helper.rs:13-29`
- **推奨修正**: `skip_next` を削除または正しく実装。

#### BUG-M06: `help` コマンドで `--all` 以外の引数を無視 【Low】
- **ファイル**: `src/main.rs:400-412`
- **推奨修正**: 不明な引数にエラーメッセージを表示。

#### BUG-M08: `build.rs` が `expect` で panic 【Low】
- **ファイル**: `build.rs`
- **推奨修正**: `expect` に具体的なメッセージを追加。

#### BUG-M09: `llvm_executer.rs` の `_full_path` パラメータが未使用 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs:23`
- **推奨修正**: パラメータを削除。

#### BUG-M10: 一時ファイル (`.ll`, `.o`, `runtime.rs`) のクリーンアップがない 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs:102-116, 121-125`
- **推奨修正**: `.ll` と `.o` を `out_dir` に書き出すか、`Debug` モード以外で削除。

#### BUG-M11: `sprs.toml` 読み込み失敗を黙殺 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs:27-28`
- **推奨修正**: エラーの種類に応じたログ出力。

#### BUG-M12: Windows 上で Linux ターゲットの実行ファイルを実行しようとする可能性 【Medium】
- **ファイル**: `src/llvm/llvm_executer.rs:189-198`
- **推奨修正**: ホストとターゲットが異なる場合は実行をスキップ。

---

## 4. 深刻度別集計 (未対応分)

| 深刻度 | 件数 | バグ ID |
|--------|------|--------|
| **Critical** | 0 | — |
| **High** | 12 | F03, F05, F10, L01, L03, L07, L08, L14, L15, L21, M03, M04 |
| **Medium** | 12 | F04, F06, F07, F08, F09, L04, L06, L09, L13, L16, L17, M12 |
| **Low** | 13 | F11, F12, F13, F14, F15, L05, L24, M05, M06, M08, M09, M10, M11 |

合計: 37 件 (ユニーク)。

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

---

*レポート作成者: sprs バグ監査チーム*

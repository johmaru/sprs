# sprs コンパイラ バグレポート (未対応分)

**対象リポジトリ**: `C:/Users/Johma_sub/sprs_new/sprs`
**最終更新**: 2026-07-22
**注記**: slab ベースのランタイム移行、M01 リファクタリング、F01/F02 修正で解消されたバグは本レポートから削除済み。以下は未対応のバグ。

---

## 1. フロントエンド (lexer / parser / AST)


#### BUG-F04b: 単項マイナス (Unary Minus) が未実装 【Low】
- **ファイル**: `src/grammar.lalrpop` (Unary 規則), `src/front/ast.rs` (Expr enum)
- **症状**: `-5` や `-x` のような単項マイナスが書けない。現在は `0 - 5` で回避している。
- **原因**: `Unary` 規則が `<p: Postfix> => p` のみで、前置 `-` (negation) の規則が無い。`Expr` にも `Neg(Box<Expr>)` バリアントが存在しない。
- **推奨修正**: `Unary` 規則に `Minus <p:Unary> => Expr::Neg(Box::new(p))` を追加し、`Expr::Neg(Box<Expr>)` を追加。compiler では `build_int_neg` で実装。


#### BUG-F06: `FunctionParam` に型フィールドがなく、関数パラメータの型注釈が不可能 【Medium】
- **ファイル**: `src/front/ast.rs`
- **推奨修正**: `ty: Option<Type>` フィールドを追加。


#### BUG-F08: `Num` / `Float` の正規表現が指数表記・16 進・`1.` 形式に未対応 【Medium】
- **ファイル**: `src/front/lexer.rs`
- **推奨修正**: Float に指数表記、Num に 16 進/2 進/8 進を追加。

#### BUG-F09: `ModuleAccess` で `base` が `Expr::Var` 以外の場合に破棄される 【Medium】
- **ファイル**: `src/grammar.lalrpop`
- **推奨修正**: `Expr::MethodCall(Box<Expr>, String, Vec<Expr>)` ノードを追加。


#### BUG-F11: `Preprocessor` トークンが `#define` のみで他の指令がエラー 【Low】
- **ファイル**: `src/front/lexer.rs`
- **推奨修正**: `#[regex(r"#[a-z]+")]` で一般化。

#### BUG-F12: `var x;` で未初期化変数が許可され、デフォルト値が未規定 【Low】
- **ファイル**: `src/grammar.lalrpop`, `src/front/ast.rs`
- **推奨修正**: Unit 型でゼロ初期化するか、型注釈必須にする。


#### BUG-F14: `ExprNoStruct` 系が `Expr` 系と重複定義 【Low】
- **ファイル**: `src/grammar.lalrpop`
- **推奨修正**: LALRPOP の `precedence` 宣言に移行。

#### BUG-F15: `Stmt` で連鎖代入 (`a = b = c;`) が不可 【Low】
- **ファイル**: `src/grammar.lalrpop`
- **推奨修正**: `Assign` を `Expr` に昇格。

---

## 2. LLVM コード生成

#### BUG-L04: `create_panic_err` が `build_call` のみで `build_unreachable` を生成しない 【Medium】
- **ファイル**: `src/llvm/value.rs`
- **推奨修正**: `create_panic_err` の最後に `build_unreachable` を追加。

#### BUG-L05: `create_add_expr` のエラーメッセージが `Box::leak` でメモリリーク 【Low】
- **ファイル**: `src/llvm/arithmetic.rs`
- **推奨修正**: モジュールの global constant として登録。

#### BUG-L06: 整数加算全般で `nsw`/`nuw` フラグ未使用、オーバーフロー時に wrap-around 【Medium】
- **ファイル**: `src/llvm/arithmetic.rs`
- **推奨修正**: 仕様に応じてオーバーフローチェックまたはフラグ追加。

#### BUG-L08: `cast` マクロの switch cases に `Int8/Int16/Int32/Int64/Uint8/...` が未登録 → 整数を cast すると f64 として誤解釈 【High】
- **ファイル**: `src/llvm/macros.rs`
- **推奨修正**: Int8/Int16/.../Uint64 のケースを追加。

#### BUG-L09: `create_field_access` で `field_index` の境界チェックなし 【Medium】
- **ファイル**: `src/llvm/data_structures.rs`
- **推奨修正**: `field_index` を検証し、範囲外なら `Err` を返す。

#### BUG-L13: `create_entry_block_alloca` が `get_insert_block().unwrap()` で panic 【Medium】
- **ファイル**: `src/llvm/value.rs`
- **推奨修正**: `None` の場合は `Err` を返す。

#### BUG-L14: `get_runtime_fn` が未知関数で `panic!` 【High】
- **ファイル**: `src/llvm/compiler.rs`
- **推奨修正**: `Result` でエラー伝播。

#### BUG-L15: `compile_block` が early-return 時にスコープをリーク / `emit_drop_for_return` の drop 順序が逆 【High】
- **ファイル**: `src/llvm/codegen.rs`, `src/llvm/compiler.rs`
- **推奨修正**: early-return 時もスコープ退出を保証。drop 順序を内側→外側に修正。


#### BUG-L17: `create_add_expr_build_float_branch` の switch default が `bb_f64` 【Medium】
- **ファイル**: `src/llvm/arithmetic.rs`
- **推奨修正**: default を `error_bb` に変更。

#### BUG-L21: `llvm_executer.rs` が `sprs.toml` の `out_dir` / `name` / `src_dir` をサニタイズせず、パストラバーサルで任意ファイル上書き 【High】
- **ファイル**: `src/llvm/llvm_executer.rs`
- **推奨修正**: `out_dir` / `name` をバリデーション。

#### BUG-L24: `PassBuilderOptions` の `run_passes` 結果を無視 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs`
- **推奨修正**: エラーをログ出力。

---

## 3. CLI / エントリポイント (`src/main.rs`, `src/command_helper.rs`, `build.rs`)

#### BUG-M03: `init_project` が既存の `sprs.toml` / `src/main.sprs` を無条件で上書き 【High】
- **ファイル**: `src/command_helper.rs`
- **推奨修正**: `Path::exists()` でチェックし `--force` フラグなしには上書きしない。

#### BUG-M04: `init_project` が `name` をサニタイズせず、パストラバーサルで任意ディレクトリにファイル作成の可能性 【High】
- **ファイル**: `src/command_helper.rs`
- **推奨修正**: `name` を `[A-Za-z0-9_-]+` にバリデーション。

#### BUG-M06: `help` コマンドで `--all` 以外の引数を無視 【Low】
- **ファイル**: `src/main.rs`
- **推奨修正**: 不明な引数にエラーメッセージを表示。

#### BUG-M08: `build.rs` が `expect` で panic 【Low】
- **ファイル**: `build.rs`
- **推奨修正**: `expect` に具体的なメッセージを追加。

#### BUG-M09: `llvm_executer.rs` の `_full_path` パラメータが未使用 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs`
- **推奨修正**: パラメータを削除。

#### BUG-M10: 一時ファイル (`.ll`, `.o`, `runtime.rs`) のクリーンアップがない 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs`
- **推奨修正**: `.ll` と `.o` を `out_dir` に書き出すか、`Debug` モード以外で削除。

#### BUG-M11: `sprs.toml` 読み込み失敗を黙殺 【Low】
- **ファイル**: `src/llvm/llvm_executer.rs`
- **推奨修正**: エラーの種類に応じたログ出力。

#### BUG-M12: Windows 上で Linux ターゲットの実行ファイルを実行しようとする可能性 【Medium】
- **ファイル**: `src/llvm/llvm_executer.rs`
- **推奨修正**: ホストとターゲットが異なる場合は実行をスキップ。

---

## 4. 深刻度別集計 (未対応分)

| 深刻度 | 件数 | バグ ID |
|--------|------|--------|
| **Critical** | 0 | — |
| **High** | 6 | L08, L14, L15, L21, M03, M04 |
| **Medium** | 9 | F06, F08, F09, L04, L06, L09, L13, L17, M12 |
| **Low** | 12 | F04b, F11, F12, F14, F15, L05, L24, M06, M08, M09, M10, M11 |

合計: 27 件 (ユニーク)。

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

---

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

### X06: `string_constants` HashMap がモジュール非スコープ 【High】

- **ファイル**: `src/llvm/compiler.rs` (string_constants フィールド), `src/llvm/value.rs` (create_panic_err)
- **症状**: 複数モジュールを import すると `Referencing global in another module!` リンクエラー
- **原因**: `create_panic_err` が `is_global: true` で `External` linkage の GlobalValue を生成し `self.string_constants` にキャッシュする。次のモジュールが同じエラーメッセージ文字列を生成すると、キャッシュから別モジュールの GlobalValue が返され、linker が別モジュールのグローバル参照として弾く
- **影響**: 複数モジュールにまたがるプログラムで `Var + Var` 等の error_bb が生成されるとリンクエラー。単一ファイルなら発生しない
- **推奨修正**: `string_constants` をモジュールごとに管理するか、`External` linkage ではなくモジュールローカルな `Internal` linkage を使用

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

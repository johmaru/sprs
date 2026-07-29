# catchable なエラー機構の設計 (Catchable Error Mechanism)

**作成日**: 2026-07-29
**ステータス**: Approved
**対象 issue**: #26
**依存 issue**: #25 (Process<T>), #27 (オーバーフロー設計) は本機構に依存

---

## 1. 目的

sprs の実行時エラー（型不一致、ゼロ除算、オーバーフロー等）を値として扱い、関数の呼び出し元で catch・回復できるようにする。

現状 `__panic`（`src/runtime/runtime.rs:875`）が `std::process::exit(1)` を呼び出しプロセス全体を即時終了する。エラーを検知して制御を取り戻す手段がない。本機構はエラーを `SprsValue` の1スロット値として表現し、`?` 演算子で伝播、マクロで catch する。

---

## 2. 路線: A（Tag::Error モノモルフィック）

`Tag::Error` を `Tag::List` や `Tag::Struct` と同じ値タグとして slab に追加する。エラーは既存の slab（`{ i32 tag, i64 data }`）の1つの値になる。ABI も型システムも変わらない。

路線B（パラメトリック `Result<T,E>`）はジェネリクス機構の全面新設が必要で、パーサ・型システム・codegen の全面改修になる。路線A は導入コストが低く、slab アーキテクチャと完全整合し、組み込み向けの速度要件を満たす。

本機構は動的ポリモルフィズムを提供する（任意の戻り値型 `>> T` の関数がエラーを返せる）。静的ポリモルフィズム（パラメトリック多相）は対象外。本 spec はエラー処理についてのみジェネリクス不要であることを示すものであり、型付きコレクション（`List<T>`）やユーザー定義ジェネリック関数/型、将来の静的型安全性など、エラー処理とは直交するジェネリクスの用途については判断しない。

---

## 3. データ表現: A2（slab ハンドル）

`Tag::Error = 9` を `Tag` enum に追加する。`data: u64` は `SlotData::Error` を指す slab ハンドル。

```rust
// runtime.rs

pub enum Tag {
    // ... 既存タグ ...
    Error = 9,
}

pub enum SlotData {
    // ... 既存バリアント ...
    Error {
        code: u32,
        message: Option<String>,
    },
}
```

`SlotData::Error` は `code: u32` と `message: Option<String>` を持つ。将来の Phase 3b で `context` フィールド拡張が可能。

---

## 4. エラーコード

### 4.1 システム定義コード（1〜99）

```rust
pub enum SprsErrorCode {
    Overflow = 1,
    DivByZero = 2,
    ModByZero = 3,
    TypeMismatch = 4,
    CastError = 5,
    ShiftTypeError = 6,
}
```

### 4.2 ユーザー定義コード（100以上）

ユーザー定義エラーコードを許可する。`@error(100, "message")` で整数リテラル指定、または `@error(MyError, "message")` で識別子指定が可能。識別子はコンパイル時にユーザー定義コード（100以上）に解決する。

ユーザー定義コードの登録機構は Phase 2 で検証する。Phase 1 では整数リテラルのみサポートし、識別子解決は将来拡張とする。

---

## 5. 伝播演算子: `?`（後置）

### 5.1 構文

```sprs
fn foo() >> i64 {
    var x = bar()?;   # bar() が Error なら foo の戻り値としてそのまま ret
    return x + 1;
}
```

### 5.2 codegen

1. `bar()` の結果 `ptr` を得る
2. `ptr` のタグをロード
3. タグ == `Tag::Error` なら → `emit_drop_for_return` → `build_return(ptr)` → 関数終了
4. タグ != `Tag::Error` なら → `ptr` をそのまま次の式で使用

エラー値は `SprsValue` なので、戻り値型 `>> T` に関わらずそのまま ret できる。これが路線A の利点。`a -> a`（エラーも同じ型として流す）も `a -> b`（エラーで別型を返す）も、型注釈を変えずに扱える。

### 5.3 AST

`Expr::Try(Box<Spanned<Expr>>)` を追加する。

### 5.4 lexer / grammar

`?` トークン（`Question`）を追加し、`Postfix` 規則に `<base:Postfix> Question => Expr::Try(Box::new(base))` を追加する。

---

## 6. 組み込みマクロ

### 6.1 `@is_error(x)` — エラー判定

タグ == `Tag::Error` なら `true`、それ以外は `false` を返す。

### 6.2 `@error_code(x)` — エラーコード取得

`SlotData::Error.code` を `u32` として返す。`x` が `Tag::Error` でない場合は `0` を返す。

### 6.3 `@error_message(x)` — エラーメッセージ取得

`SlotData::Error.message` を `String` として返す。`message` が `None` の場合は空文字列を返す。`x` が `Tag::Error` でない場合は空文字列を返す。

### 6.4 `@error(code, message)` — エラー値生成

`Tag::Error` の `SprsValue` を生成する。`code` は整数リテラルまたは識別子（Phase 1 では整数リテラルのみ）。`message` は文字列リテラル。

```sprs
fn validate(x) >> i64 {
    if x < 0 {
        return @error(100, "x must be non-negative");
    }
    return x;
}
```

---

## 7. error short-circuit ルール

全タグディスパッチサイト（`create_add_expr`, `create_minus_expr`, `create_mul_expr`, `create_div_expr`, `create_mod_expr`, `@cast`, `@lshift`/`@rshift`）のタグディスパッチ冒頭で、以下のルールを適用する:

> **オペランドのいずれかが `Tag::Error` なら、そのエラー値をそのまま伝播する。新規エラーを生成しない。**

### 7.1 理由

`var x = might_overflow() + 1;`（`?` なし）のとき、`might_overflow()` が `Tag::Error`（`Overflow`）を返した場合、`create_add_expr` のタグディスパッチで int/float/string のいずれにも該当せず `error_bb` に落ちる。short-circuit ルールがないと、ここで新規 `TypeMismatch` エラーが生成され、元の `Overflow` コードが上書きされて消える。デバッグ不能になる。

### 7.2 codegen

各演算のタグ比較の前に、以下の2分岐を冒頭に追加する:

1. `l_tag == Tag::Error` → `l_val`（左オペランドの値）をそのまま返す
2. `r_tag == Tag::Error` → `r_val`（右オペランドの値）をそのまま返す

両方が `Tag::Error` の場合は左を優先する（最初にチェックした方）。

### 7.3 対象サイト

- `arithmetic.rs`: `create_add_expr`, `create_minus_expr`, `create_mul_expr`, `create_div_expr`, `create_mod_expr`
- `macros.rs`: `call_builtin_macro_cast`, `call_builtin_macro_lshift`, `call_builtin_macro_rshift`

---

## 8. `create_panic_err` 移行

現状6箇所の `create_panic_err` + `build_unreachable` を `Tag::Error` セットに置換する。

| ファイル | 行 | 現状 | 移行後エラーコード |
|---|---|---|---|
| `arithmetic.rs` | 124 | 加算型不一致 → `__panic` | `TypeMismatch` |
| `arithmetic.rs` | 619 | `@cast` 型エラー → `__panic` | `CastError` |
| `arithmetic.rs` | 1608 | ゼロ除算 → `__panic` | `DivByZero` |
| `arithmetic.rs` | 1686 | ゼロ剰余 → `__panic` | `ModByZero` |
| `macros.rs` | 296 | `@cast` 型エラー → `__panic` | `CastError` |
| `macros.rs` | 813 | シフト型エラー → `__panic` | `ShiftTypeError` |

各サイトの `build_unreachable()` を削除し、エラー値をセットした後に通常の制御フローに戻る（エラー値が呼び出し元に伝播する）。

---

## 9. `__panic` の位置づけ

- `__panic` は「catch されなかったエラーが最終的に到達する回復不能パス」として残す
- `main` 関数のトップレベルでエラーが伝播しきった場合、`__panic` を呼んでプロセス終了（`exit(1)`）
- `__panic` 自体の実装は現状通り
- 将来 `Process<T>`（#25）導入時は、プロセス境界で catch されなかったエラーが `__panic` に届く

---

## 10. 型注釈との関係

sprs の型注釈 `>> T` は現状「書けるけど無視される」（BUG-F06）。エラー機構導入後も:
- `>> i64` は正常時の戻り値型を示す
- エラー値は `Tag::Error` なので、`>> i64` 関数がエラーを返しても型チェックでは通る（動的型付けの一環）
- 将来的に静的型チェックを導入する際、`>> i64 | error` のような構文を追加可能

---

## 11. 実装対象ファイル

### 11.1 runtime 層

- `src/runtime/runtime.rs` — `Tag::Error = 9`、`SlotData::Error`、`__error_new`、`__is_error`、`__error_code`、`__error_message` 追加。`__clone` に `Tag::Error` 分岐追加。`format_sprs_value` に `Tag::Error` 表示追加。`is_heap_tag` に `Tag::Error` 追加
- `src/llvm/compiler.rs` — `Tag::Error` を `Tag` enum に追加（runtime.rs と同期）、`get_runtime_fn` に新関数登録

### 11.2 codegen 層

- `src/llvm/value.rs` — `create_panic_err` を `create_error_value`（`Tag::Error` をセットする IR 生成）に置換。`PanicErrorSettings` → `ErrorValueSettings` に改名
- `src/llvm/arithmetic.rs` — 6箇所の panic サイトを `create_error_value` に置換、`build_unreachable` 削除。全タグディスパッチに short-circuit ルール追加
- `src/llvm/macros.rs` — 2箇所の panic サイトを `create_error_value` に置換、`build_unreachable` 削除。`@cast`/`@lshift`/`@rshift` に short-circuit ルール追加。`@is_error`/`@error_code`/`@error_message`/`@error` マクロ追加
- `src/llvm/codegen.rs` — `?` 演算子の codegen（`Expr::Try` 分岐）、`@error`/`@is_error`/`@error_code`/`@error_message` マクロのディスパッチ追加

### 11.3 フロントエンド層

- `src/front/ast.rs` — `Expr::Try(Box<Spanned<Expr>>)` 追加
- `src/front/lexer.rs` — `Question` トークン（`?`）追加
- `src/grammar.lalrpop` — `Postfix` 規則に `<base:Postfix> Question => Expr::Try(Box::new(base))` 追加

### 11.4 トップレベル

- `src/main.rs` / `src/llvm/llvm_executer.rs` — `main` 関数のトップレベルでエラーが伝播しきった場合の `__panic` 呼び出し

---

## 12. 実装フェーズ

### Phase 1: runtime + codegen 基盤

1. `Tag::Error = 9`、`SlotData::Error` 追加
2. `__error_new`、`__is_error`、`__error_code`、`__error_message` runtime 関数追加
3. `create_error_value`（`create_panic_err` 置換）実装
4. 6箇所の panic サイトを `create_error_value` に置換、`build_unreachable` 削除
5. `__clone` に `Tag::Error` 分岐追加
6. `format_sprs_value` に `Tag::Error` 表示追加
7. `is_heap_tag` に `Tag::Error` 追加

### Phase 2: short-circuit ルール

8. `create_add_expr` に short-circuit 追加
9. `create_minus_expr`, `create_mul_expr`, `create_div_expr`, `create_mod_expr` に short-circuit 追加
10. `@cast`, `@lshift`, `@rshift` に short-circuit 追加

### Phase 3: `?` 演算子

11. `Question` トークン追加（lexer）
12. `Expr::Try` AST ノード追加
13. grammar 規則追加
14. `?` の codegen 実装

### Phase 4: エラーマクロ

15. `@is_error` マクロ実装
16. `@error_code` マクロ実装
17. `@error_message` マクロ実装
18. `@error` マクロ実装

### Phase 5: main 境界の `__panic`

19. `main` 関数のトップレベルでエラー伝播しきった場合の `__panic` 呼び出し

---

## 13. 検証

### 13.1 既存テストの回帰

全テストスイート（87 PASS / 7 XFAIL / 8 FAIL）が回帰しないこと。panic サイトが `Tag::Error` に変わっても、エラー値が最終的に `__panic` に到達して `exit(1)` するため、既存の XFAIL/FAIL テストの挙動は変わらないはず。

### 13.2 新規テスト

- `@is_error` / `@error_code` / `@error_message` でエラーを catch できること
- `?` でエラーが伝播すること
- `@error(100, "msg")` でユーザー定義エラーを生成できること
- short-circuit ルール: `might_overflow() + 1` で元のエラーコードが保持されること
- `var x = might_fail(); if @is_error(x) { ... } else { ... }` で回復できること

### 13.3 Phase 3a（将来）

`__panic` の ABI 拡張（file/line/col 静的引数埋め込み）は構造化エラーレポート spec の Phase 3a で別途対応。

---

## 14. 非目標 (Non-Goals)

- **ジェネリクス機構の導入**: 本 spec の対象外。路線A はモノモルフィック
- **静的型チェック**: `>> T | error` のような構文は将来拡張
- **`Process<T>`（#25）**: プロセス間層は別 issue。本 spec はプロセス内層のみ
- **オーバーフロー設計（#27）**: `Overflow` エラーコードは定義するが、`@AnyError`/`@WhatError` 構文の詳細設計は #27 で対応
- **ユーザー定義エラー識別子解決**: Phase 1 では整数リテラルのみ。識別子（`@error(MyError, ...)`）は将来拡張

---

*Spec author: sprs design session*
*Date: 2026-07-29*

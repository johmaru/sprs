# 型と束縛

## 正規の型

表面の型名は一表記だけです。旧別名（`int`、`fp`、`fp16`/`fp32`/`fp64`、`list`、`range`、`buffer`、`rawptr`、`err`、`atom`、型位置の `label`）は `SPRS-SEM-011` で、`help` に置換先を付けます。

| 表面 | 意味 |
|------|------|
| `i8` `u8` `i16` `u16` `i32` `u32` `i64` `u64` | 整数幅。注釈のない整数リテラルは `i64`。 |
| `f16` `f32` `f64` | 浮動小数点幅。注釈のない浮動リテラルは `f64`。`@cast` もこの名前。 |
| `bool` | 真偽値 |
| `str` | 文字列 |
| `unit` | Unit（`()`） |
| `Any` | 検査しない / 動的 |
| `List(T)` | リスト。引数は常に 1 つ（要素が不明なら `List(Any)`）。裸の `List` / `List()` は拒否。 |
| `Range` | Range |
| `Buffer` | 固定長でゼロ初期化されたバイト配列 |
| `RawPtr` | `@raw(buf)` から得られる素のアドレス |
| `Label` | 広いラベル。payload なし atom と payload 付きラベルの両方 |
| `:name` | 正確な payload なし atom（`:ready`） |
| `Label(:name, T)` | 正確な payload ラベル。第 1 引数は `:name`。 |
| PascalCase 名 | 構造体または閉じたラベル集合（`Point`、`ConnectionState`） |

`Type::Label` は表面の合併型です。単一のランタイムタグを仮定しません（`tag_discriminant` は `None`）。ランタイムは payload なし atom に `Tag::Atom = 9`、payload 付き値に `Tag::Label = 10` を使います。これらは実装詳細であり型名ではありません。

推論では payload なし `:ready` は `:ready`、payload 付き `{:ok, 7}` は名前が静的なら `Label(:ok, i64)`、動的なら広い `Label` です。

広い `Label` は正確な atom、payload ラベル、閉じたラベル集合、ランタイム専用 atom タグを受理します。正確な `:name` は名前一致だけです。`Label(:name, T)` は名前と payload 型を再帰比較します。`Label(:ok)`（1 引数）と `Atom(...)` は拒否します。

`Self` は構造体フィールド型の内側でのみ有効です（`List(Self)` を含む）。[構造体](structs.md) を参照してください。

型の適用はコンパイル時だけです: `List(i64)`、`Label(:ok, i64)`、`Label(:error, Any)`。

## コメントとリテラル

行コメントは `#` から行末までです。
`#define Windows` と `#define Linux` はプリプロセッサトークン（`priority = 4`）であり、通常のコメントより優先されます。
`#define` は [モジュール](../reference/modules.md) を参照してください。

整数リテラルは `[0-9]+` に一致し、`i64` です。

浮動小数点リテラルは `[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?` と `[0-9]+[eE][+-]?[0-9]+` に一致し、`f64` です。

文字列リテラルは `"..."` です。
エスケープは `\n`、`\t`、`\r`、`\0`、`\\`、`\"`、`\'`、`\u{XXXX}`（16進 1〜6 桁）です。
未知のエスケープと末尾の `\` は、書いたとおりに残します。

真偽値は `true` と `false` です。

リストリテラルは、「変数と代入」の `[1, 2, 3]` 例と同じ形です。

## 識別子と命名（`SPRS-SEM-025`）

識別子は `[A-Za-z_][A-Za-z0-9_]*` です。末尾の `!` は名前の一部ではありません。

ASCII のみ。数字は 2 文字目以降に置けます。略語は通常の語として扱います（`DmaController`。`DMAController` ではない）。ユーザー定義のどの分類でも `__` 接頭辞は拒否します。単独の `_` は match のワイルドカードだけです。`_name` はローカル変数、パラメータ、パターン束縛だけに許可します。

| 分類 | スタイル | 例 |
|------|----------|----|
| 関数、モジュール/`pkg`/`import`、グローバル `var`、フィールド、attach スロット、マクロ | `snake_case` | `start_dma`、`fn_builds`、`@buf_len` |
| ローカル変数 / パラメータ / 束縛 | `snake_case`、任意で `_name` | `item`、`_tmp` |
| 構造体、閉じたラベル集合、FunctionBuild、名前付き型、型パラメータ | `PascalCase` | `Point`、`T`、`DmaController` |
| 開いたラベル（`:name`）と閉じたメンバー | `snake_case` | `:ready`、`:ConnectionState.waiting_for_dma` |

静的ラベルは分解して検査します。`:ready` は snake_case、`:ConnectionState.waiting_for_dma` は集合名 PascalCase + メンバー snake_case。動的 `:"..."` の文字列は検査しません。正規名の自動生成 / リネームは未実装です。

違反は `SPRS-SEM-025` で、分類別メッセージ（例: `function names must use snake_case`）を出します。

## キーワードと `^`

キーワードは常にキーワードです。識別子位置の裸のキーワードは `SPRS-SYN-002`（`` `keyword` is a reserved keyword ``）で、`help: use ^keyword if this name is intentional` です。

キーワードを名前にするには `^` でエスケープします。`^` は名前の一部ではありません。`new(expr)`、`destroy(expr)`、`exist(expr)`、`defer expr;` はコア構文のままです。同じ綴りを識別子にするときは `^new` などが必要です。

`@name` はマクロトークンです。`@^name` はレキサーエラーです。

分類規則を満たす非キーワードをエスケープすると `SPRS-SYN-008`（`` unnecessary identifier escape `^foo` ``）で、`help: use foo instead of ^foo` です。`^BadName` をローカルに使う場合は不要エスケープより `SPRS-SEM-025` を先に出します。

キーワード: `if` `else` `while` `fn` `use` `function_build` `private` `return` `pkg` `import` `var` `pub` `struct` `ambi` `new` `destroy` `exist` `unsafe` `defer` `match` `case` `break` `true` `false` `bool` `str` `unit` `i8`…`u64` `f16` `f32` `f64` `label` `init` `source` `params` `return_type` `visibility` `type_param` `when` `is` `neq` `and` `or` `not`。`label` は宣言専用（`label Color { ... }`、`label :ready;`）。型位置は `Label` です。型位置に残るキーワードは上記のプリミティブ型名と `ambi` だけです。

単独の `^`、`^1`、`^^name` はレキサーエラーです。

## 変数と代入

```sprs
# Comments start with a hash symbol
var x = 10;
var name = "sprs";
var is_valid = true;
var numbers = [1, 2, 3];


# Not initialized variable
var y;  # y is initialized to Unit type

# Re-assignment

var y;
y = 20;
y = "now a string"; # y is now a string

```

ヒープ値は代入、呼び出し、`return` でムーブします。コピーを残すときは `@clone` です。[メモリ管理](../reference/memory-management.md) を参照してください。

## 型付き束縛と `ambi`

パラメータと戻り値の型は `>>` 注釈を使います。
注釈のないパラメータは動的なままです。
注釈付きパラメータは呼び出し側で検査されます（引数個数と型）。

```sprs
fn add(a >> i64, b >> i64) >> i64 {
  return a + b;
}
```

固定注釈は、互換しない再代入を拒否します。
束縛をその型で始めつつ動的な再代入を許すときは、型の前に `ambi`（ambiguous）を付けます。

```sprs
fn demo(fixed >> i64, flex >> ambi i64) {
  fixed = 1;      # ok
  flex = 1;       # ok
  flex = "x";     # ok — becomes dynamic after reassignment
}
```

適用型は入れ子になり、コンストラクタ名と各引数で検査されます。

```sprs
fn take(xs >> List(i64)) >> List(i64) {
  return xs;
}

fn parse() >> Label(:error, Any) {
  return {:error, "no"};
}
```

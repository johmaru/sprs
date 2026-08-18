# 型と束縛

## 基本データ型

- Int (i64) — 注釈キーワード `int`（型検査では `i64` と互換）
- Float (f64) — 注釈キーワード `fp`（型検査では `fp64` / `f64` と互換）
- Bool — `bool`
- Str — `str`
- List（動的配列） — 注釈キーワード `list`（`List(T)` 適用形もある）
- Range — `range`
- Unit — `unit`
- Enum — コンパイル時フレーム。
  バリアントは Atom（`Color.Red`）
- Struct
- Buffer — 固定長でゼロ初期化されたバイト配列。
  注釈キーワード `buffer`
- RawPtr — `@raw(buf)` から得られる素のアドレス。
  注釈キーワード `rawptr`
- エラーラベル（捕捉可能） — `Label(:error, any)` の糖衣構文 `err`
- Atom（不変な名前） — 注釈キーワード `atom`（`Atom(:name)` 適用形もある）
- Label（タグ付き値） — 注釈キーワード `label`（`Label(:name[, T])` 適用形もある）
- i8 / u8 / i16 / u16 / i32 / u32 / i64 / u64（主に `@cast`。
  `>>` 注釈でも使える）
- fp16 / fp32 / fp64（主に `@cast`。
  `>>` 注釈でも使える）


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

注釈での型の *適用* は `Name(Type, …)` です（例: `List(int)`、`Result(int, err)`、`Label(:ok, int)`）。
これらはコンパイル時の形だけで、ランタイムタグではありません。

日常のコードでは平坦なキーワード（`list`、`err`、`atom`、`label`、`buffer`、`rawptr`）を使います。
ジェネリクス / 型パラメータ（`Param`）は、まだユーザー向けではありません。

## キーワード識別子

キーワードはレキサートークンのままですが、識別子が期待される多くの場所では名前として構文解析されます（`pkg`、`import`、`fn` 名、パラメータ、`var`、フィールド、呼び出し、変数参照）。

エスケープなしで名前として使えます（例: `pkg buffer;`、`fn buffer(new >> int)`、`var rawptr = new;`）:
`fn`、`case`、`break`、`pkg`、`import`、`var`、`pub`、`enum`、`struct`、`cp`、`ambi`、
`unsafe`、`int`、`fp`、`bool`、`str`、`list`、`buffer`、`rawptr`、`range`、`unit`、
`err`、`label`、`atom`。
パラメータ名は `new` / `destroy` / `exist` でも構いません。
`var defer = …` は許可されます。
式のなかの裸の `new` / `destroy` / `exist` は変数です。
`new(4)` / `destroy(x)` / `exist(x)` はヒープ形式のままです。

裸で書くと構文のままになるもの: `if`、`else`、`while`、`match`、`return`、`true`、
`false`。
`defer` は `var` 名としてのみ使えます（式の識別子ではありません）。
`i8`…`u64` / `fp16`…`fp64` は `@cast` 用の型アトムのままです。
`>>` 位置では型キーワードは型のままです（`>> buffer` は Buffer であり、名前付き型ではありません）。

## `^` エスケープ

任意の識別子の先頭に `^` を付けると、裸では使えないキーワードを含め、通常の名前として扱います。
`^` は名前の一部ではありません。
`^fn` と `fn` は同じ識別子です。
例: `var ^fn = 1;` / `^fn = ^fn + 1;`。
単独の `^`、`^1`、`^^fn` はレキサーエラーです。

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

## 型付き束縛と `ambi`

パラメータと戻り値の型は `>>` 注釈を使います。
注釈のないパラメータは動的なままです。
注釈付きパラメータは呼び出し側で検査されます（引数個数と型）。
検査では `int` と `i64` は同じ型として扱います。
`fp` と `fp64` も同様です。

```sprs
fn add(a >> int, b >> int) >> int {
  return a + b;
}
```

固定注釈は、互換しない再代入を拒否します。
束縛をその型で始めつつ動的な再代入を許すときは、型の前に `ambi`（ambiguous）を付けます。

```sprs
fn demo(fixed >> int, flex >> ambi int) {
  fixed = 1;      # ok
  flex = 1;       # ok
  flex = "x";     # ok — becomes dynamic after reassignment
}
```

適用型は入れ子になり、コンストラクタ名と各引数で検査されます。

```sprs
fn take(xs >> List(int)) >> List(int) {
  return xs;
}

fn parse() >> Result(int, err) {
  return 1;
}
```

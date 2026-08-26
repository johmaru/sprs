# 構造体

裸の構造体名は、フィールド、関数引数、戻り値の型注釈で使えます。
`Self` は宣言中の構造体へ解決され、その構造体のフィールド型の内側でのみ有効です。
`List(Self)` のような入れ子の型適用も含みます。
同一モジュール内の構造体型は、宣言順に関係なく参照できます。
未定義の裸の型名、または構造体フィールドの外の `Self` は `SPRS-SEM-011` として報告されます。

```sprs
struct Tree {
  value >> i64,
  children >> List(Self)
}

fn identity(value >> Tree) >> Tree {
  return value;
}
```

## `init`

構造体値は `init TypeName { field = expr, ... }` で作ります。末尾カンマは許可します。
ゼロフィールド構造体は `init Empty {}` です。

フィールドは宣言順に埋めます。明示した初期化子が優先です。省略したフィールドは
`StructField.default_value` を、各 `init` の呼び出し側の式コンテキストで評価します。
default 式に `self` や先行フィールドの束縛はありません。default も初期化子もない
フィールドは `missing required field \`name\` in init Type` です。未知フィールドと
重複フィールドは初期化子の span でコンパイルエラーです。旧 `@init(...)` は構造体
初期化ではありません。`@init` は未知マクロです。

```sprs
pub struct Point {
  x >> i64,
  y >> i64
}

struct Counter {
  value >> i64 = 0,
  name >> str
}

struct Empty {}

fn main() {
  var p = init Point {
    x = 10,
    y = 20,
  };
  @println(p.x);
  @println(p.y);

  var c = init Counter {
    name = "counter",
  };
  @println(c.value); # default の 0

  var e = init Empty {};
}
```


## ジェネリック構造体

構造体名の直後に PascalCase の型パラメータを置けます。例: `struct Pair(T)`、
`struct Pair(A, B)`。フィールド型はそのパラメータを参照できます。
ジェネリック構造体は `Pair(i64)` や `init Pair(str) { ... }` のように、
明示した具象適用でのみ使います。コンパイラは引数をコンパイル時に置換し、
引数リストごとに別の具象レイアウトを作ります（`Pair(i64)` と `Pair(f64)` は別です）。
`Pair(Pair(i64))` のような入れ子は内側を先に特殊化します。

所有文字列の特殊化は `str` です（`init Pair(str) { a = "owned", b = "x" }`）。
`String` は型名ではありません。ジェネリックフィールドの default、期待型だけによる
型引数なしの `init Pair { ... }`、ジェネリックメソッドは現在使えません。

```sprs
struct Pair(T) {
  a >> T,
  b >> T
}

fn use_pair() {
  var nums = init Pair(i64) { a = 1, b = 2 };
  var owned = init Pair(str) { a = "owned", b = "x" };
  var nested = init Pair(Pair(i64)) {
    a = init Pair(i64) { a = 1, b = 2 },
    b = init Pair(i64) { a = 3, b = 4 },
  };
}
```

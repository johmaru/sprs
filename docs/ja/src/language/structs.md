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

# 列挙型と構造体

## Enum フレーム

```sprs
pub enum Animal {
 Dog,
 Cat,
}

fn main() {
   @println(Animal.Dog);

}

```

ソースの `enum` はコンパイル時フレームだけです。
`Color.Red` はランタイム Atom で、intern キーは `"Color.Red"` です（即時 intern id であり、slab ハンドルではありません）。
ランタイムタグ `7`（かつての `Tag::Enum`）は未使用です。

- `Color.Red == Color.Red` は真です。
  `Color.Red == :Red` は偽です（キー `"Color.Red"` 対 `"Red"`）。
- enum 型の被検査値に対する `match` は `case :Red`（裸のバリアント）を使います。
  コンパイラはフレーム化されたキーと比較し、すべてのバリアントか末尾の `case _` を要求します
  （エラー `non-exhaustive match on Color; missing Green, Blue`）。
  開いた Atom / Label の match はランタイム検査のままです（`Match failed`）。
- 同一コンパイル内での重複 `enum` 名は意味エラーです。
  非 `pub` バリアントは他モジュールから見えません。

```sprs
pub enum Color {
  Red,
  Green,
  Blue,
}

fn enum_match_red() >> int {
  var r = match Color.Red {
    case :Red => 1
    case :Green => 2
    case :Blue => 3
  };
  return r;
}
```

## グループ化された label による enum 互換宣言

グループ化された `label` 宣言は、名前空間付き Atom 向けの enum 互換構文を提供します。
`pub label :Color{:red, :blue}` はソース `enum` と同じ種類のコンパイル時フレームを作り、それをエクスポートし、バリアントを `Color.red` と `Color.blue` として公開します。
どちらの宣言形式も、実行時にはフレーム化された Atom intern キーを生成します。

```sprs
pub label :Color{:red, :blue}

fn print_grouped_label_color() {
  @println(Color.red);
}
```

## 構造体

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

```sprs
pub struct Point {
  x >> i64,
  y >> i64
}

fn main() {
 var p = @init(Point {
  x = 10,
  y = 20
 });

@println(p.x); # prints 10
@println(p.y); # prints 20
}
```

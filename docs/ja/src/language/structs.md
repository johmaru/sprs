# 構造体

裸の構造体名は、フィールド、関数引数、戻り値の型注釈で使えます。
`Self` は宣言中の構造体へ解決されます。その構造体のフィールド型（`List(Self)` のような入れ子の型適用を含む）、および method のパラメータ / 戻り値注釈と本体で有効です。
同一モジュール内の構造体型は、宣言順に関係なく参照できます。
未定義の裸の型名、または構造体フィールドと method の外の `Self` は `SPRS-SEM-011` として報告されます。
値そのものによる field 循環（`struct A { x >> A }` や、`Ptr` / `List` などの間接型を挟まない相互参照）も `SPRS-SEM-011`（`recursive struct has infinite storage size`）です。
前方参照は、循環が `Ptr(T)` や `List(T)` を通るときは有効です。

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
重複フィールドは初期化子の span でコンパイルエラーです。旧 `@init(...)` は構造体初期化ではありません。
`init Type { ... }` が構造体を作り、`@init(*p, value)` は `Ptr(MaybeUninit(T))` storage の初期化です。RuntimeValue の `Unit` スロット書き込みではありません。

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
`String` は型名ではありません。ジェネリックフィールドの default と、期待型だけによる
型引数なしの `init Pair { ... }` は現在使えません。

構造体の typed storage（`StorageRep`）は、target ABI の padding を含む inline の field layout です。
通常の構造体値はまだ RuntimeValue の `Tag::Struct` slab ハンドルで評価されることがあります。
storage との pack / unpack はコンパイラとランタイムの橋渡しです。
ポインタ算術と `@init` / `@take` が使うのは inline layout であり、`{tag,data}` スロットではありません。
[型と束縛](types-and-bindings.md) を参照してください。

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

## メソッド

メソッドは構造体の内側、フィールドの後に宣言します。メソッドがある場合は最後のフィールドにもカンマが必要です。最初のパラメータは注釈なしの `self` でなければなりません。レシーバは通常の関数引数と同じく move されます。暗黙の借用や clone はありません。

```sprs
struct Pair(T) {
  a >> T,
  b >> T,
  pub fn get(self) >> T {
    return self.a;
  }
}

fn main() {
  var pair = init Pair(i64) { a = 1, b = 2 };
  pair.get();
  make_pair().get();
}
```

`Self` と owner の型パラメータは具象 owner（`init Pair(i64) { ... }` なら `Pair(i64)`）へ置換されます。
入れ子の method では FunctionBuild、static method、overload、method 固有の型パラメータは使えません。
struct でないレシーバへの method 呼び出しは `SPRS-TYP-007`（`method call requires a struct receiver, found X`）です。

公開されるのは public 構造体の public method だけです。
private method は宣言モジュール内でのみ呼び出せます。
import 先から private method を呼ぶと `SPRS-SEM-015`（`Undefined function: {name}`）になります。

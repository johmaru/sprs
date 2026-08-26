# メモリ管理

Sprs はヒープ値（`str`、`List`、`Range`、構造体、ラベル、`Buffer`）に **ムーブセマンティクス** を使います。
これらの値を代入または渡すと所有権が移り、古い束縛は無効になります（`Unit`）。
整数、浮動小数点、bool はコピーされます。

ムーブになる利用のあと元の値を残すときは `@clone(x)` です。変数から明示的にムーブするときは
`@move(x)` です（束縛は `Unit` になります）。`cp var` はありません。通常の `var` は常にムーブします。

**リスト添字はムーブ:**

`values[index]` は要素の所有権をムーブし、list 側のそのスロットを `Unit` にします。
同じ index を再読すると `Unit` になります。
元の list の要素を残したい場合は、先に list 自体を `@clone` し、clone 側から取得します。

```sprs
fn main() {
    var values = [];
    @list_push(values, "hello");
    var first = values[0];     # moves the string out; values[0] is now Unit
    @println(first);           # prints: hello
    @println(values[0]);       # prints: ()
}
```

**代入時のムーブ:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    var copy = greeting;       # ownership moves to copy; greeting is now invalid
    @println(copy);            # prints: Hello, Sprs!
}
```

**関数呼び出しへのムーブ:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    @println(greeting);        # greeting is moved into @println and becomes invalid
}
```

**`@clone` で所有権を残す:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    @println(@clone(greeting)); # prints a copy; greeting stays valid
    @println(greeting);         # still prints: Hello, Sprs!
}
```

**明示的な `@move`:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    @println(@move(greeting));  # greeting becomes Unit
}
```

**型付きポインタの間接参照（`Ptr(T)`）:**

所有しない読み取り `*p` では pointee はその場に残ります。所有権を受け取る文脈 — 変数初期化（`var x = *p`）、引数、`return`、list / label / 構造体への格納 — では既存の `__clone` 経路で pointee を clone します。

置換代入 `*p = value` は右辺の所有値を先に用意し、古い pointee を drop してから格納します。そのため `*p = *p` は drop の前に clone します。ポインタ値そのものは所有しないアドレスであり、追加の drop 対象にはしません。

ポインタ place の所有権は次の 3 契約です。

- `@move(*p)` は `{tag,data}` を取り出し、先のスロットをヒープ／プリミティブ問わず `Unit` にする。
- `@init(*p, value)` は `Unit` のスロットにだけ格納する。古い値の `__drop` は呼ばない。`Unit` 以外なら `@init destination is already initialized` で panic する。
- `*p = value` は置換。右辺を所有値化してから古い pointee を `__drop` し、格納する。

Buffer は他のヒープ値と同じ自動 drop 経路に参加します。
明示的な寿命切断が必要なときは `destroy` / `defer destroy(...)` を使ってください。
Buffer の生存、`unsafe`、RawPtr、`defer` の順序の詳細は [Buffer と unsafe](../language/buffers-and-unsafe.md) にあります。

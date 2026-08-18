# メモリ管理

Sprs はヒープ値（`str`、`list`、`range`、`struct`、`label`、`buffer`）に **ムーブセマンティクス** を使います。
これらの値を代入または渡すと所有権が移り、古い束縛は無効になります（`Unit`）。
整数、浮動小数点、bool はコピーされます。

ムーブのあとも元の値を残したいときは `@clone(x)` を使います。
同じ束縛を何度も読み、毎回 `@clone` を書くのが煩わしいときは `cp var` を使います。
1 回分その糖衣を外すには `@move(x)` を使います。

`cp` による自動クローンは、そうしなければ所有権がムーブされるときに働きます。
関数引数、`@println` / `@list_push`、代入の右辺、別変数からの `var` / `cp var` 初期化、`return` です。
すべての式オペランドを書き換えるわけでは **ありません**（例: `a + b`）。

**Phase 1:** `cp` は主に `str` 向けです。
他のヒープ型でも動きますが、使用ごとにディープコピーします。
`cp` が明らかに `list` / `range` / `struct` に適用されていると、コンパイラは警告します。

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

**`cp var` による常時クローン束縛:**

```sprs
fn main() {
    cp var greeting = "Hello, Sprs!";
    @println(greeting);         # same as @println(@clone(greeting))
    @println(greeting);         # still valid
    @println(@move(greeting));  # one-shot real move; greeting becomes Unit
}
```

Buffer は他のヒープ値と同じ自動 drop 経路に参加します。
明示的な寿命切断が必要なときは `destroy` / `defer destroy(...)` を使ってください。
Buffer の生存、`unsafe`、RawPtr、`defer` の順序の詳細は [Buffer と unsafe](../language/buffers-and-unsafe.md) にあります。

# 組み込みマクロ

マクロ名の形は `@[A-Za-z_][A-Za-z0-9_]*` です。
単独の `@` はレキサーエラーです。
未知の名前は `SPRS-SEM-003`（`Unknown macro: ...`）です。

* `@println(value)`: 値をコンソールに表示する

例:

```sprs
@println(y[1]);
```

* `@list_push(list, value)`: リスト末尾に値を追加する。`List(T)` では値は `T` に代入可能でなければならない。`List(Any)` は任意の要素を許す。結果型は `unit`。`@clone` / `@move` は引数の静的型（`List(T)` など）を保持する。

例:

```sprs
@list_push(y, z);
```

* `@buf_len(buf)`: Buffer 長を Integer として返す（古い / 非 Buffer では `0`）
* `@buf_get(buf, i)`: 1 バイトを Integer として読む。
  OOB / 古い → `Unit`
* `@buf_set(buf, i, v)`: `v` の下位 8 ビットを `i` に書く。
  OOB → 何もしない

例:

```sprs
var a = new(2);
@buf_set(a, 0, 7);
@println(@buf_get(a, 0));
@println(@buf_len(a));
```

Buffer の割り当て、添字、`destroy`、`exist`、`unsafe`、RawPtr、`defer` は [Buffer と unsafe](../language/buffers-and-unsafe.md) を参照してください。

* `@raw(buf)`: Buffer の所有権を RawPtr へムーブする。
  `unsafe { ... }` が必要。
  元の束縛は `Unit` になり、呼び出し側は結果を `@free` しなければならない。
* `@free(p)`: `@raw` 由来の RawPtr を解放する。
  `unsafe { ... }` が必要。
  null / 未知のアドレスは何もしない。
  元の束縛は `Unit` になる。

例:

```sprs
var b = new(2);
unsafe {
  var p = @raw(b);
  @free(p);
}
@println(exist(b)); # false
```

* `@clone(value)`: 値をクローンする

例:

```sprs
var a = "hello";
@println(@clone(a));

```

* `@move(value)`: 変数からムーブする。元の束縛は `Unit` になる。`@move(*p)` はコンパイルエラー。raw storage には `@take` を使う。

例:

```sprs
var a = "hello";
@println(@move(a)); # a becomes Unit
```

* `@init(*p, value)`: 未初期化 storage へ `value` をムーブする。`p` は `Ptr(MaybeUninit(T))` で、第一引数は `*p` でなければならない。結果型は `unit`。置換代入ではなく、古い値は drop しない。構造体初期化（`init Type { ... }`）ではない。

```sprs
@init(*p, x);
```

* `@ref(*p) -> Ptr(T)`: `p : Ptr(MaybeUninit(T))` に現在有効な `T` があると caller が保証する。所有権は動かず、`p` の型も変わらない。戻り値は `Ptr(T)`。

* `@take(*p) -> T`: `p : Ptr(MaybeUninit(T))` から有効な `T` をムーブする。元のバイト列は論理的に未初期化になり、`Unit` は書き込まない。

ムーブセマンティクス、`@clone`、`@move`、`@init` / `@ref` / `@take`、自動 drop は [メモリ管理](memory-management.md) を参照してください。

* `@cast(value, type)`: 値を指定型へキャストする

例:

```sprs
var a = 100; # default is i64
var b = @cast(a, i8); # cast to i8
@println(b); # prints 100 as i8
```

* `@fcast(value)`: `int`、`bool`、`str` の値を明示的に `str` へ変換する。
  未対応の値は捕捉可能なエラー `TypeError: unexpected tag in @fcast` を返す。
  既存のエラーはそのまま返す。
  暗黙の文字列変換は行わない。

```sprs
var ok = 5 == 5;
@println("bool test : " + @fcast(ok)); # bool test : true
```

* `@lshift(value, shift_amount)`: 引数はちょうど 2 つ。
  Integer タグのみ。
  符号付きタグ（`Integer`、`i8`、`i16`、`i32`、`i64`）は `shl` / 算術右シフトを使う。
  符号なしタグ（`u8`..`u64`）は `shl` / 論理右シフトを使う。
  非整数値はエラーラベル `"@lshift expects an integer value"` を生成する。
  既存のエラーラベル引数はそのまま返す。
  結果は `value` のタグを保つ。
* `@rshift(value, shift_amount)`: `@lshift` と同じ規則。
  非整数のメッセージは `"@rshift expects an integer value"`。
* `@not(value)`: 引数はちょうど 1 つ。
  `data == 0` のとき Boolean `true`、それ以外は `false`。
  ビット単位の補数ではない。
構造体初期化はコア構文 `init TypeName { field = value, ... }` であり、マクロではありません。
旧構造体 `@init(...)` は廃止です（`SPRS-SEM-003`）。ポインタ `@init(*p, value)` は上で説明しています。
[構造体](../language/structs.md) を参照してください。


* `@attach(expr, <:name)`: `expr` をクローンして関数局所の attach スロット `<:name` へ入れる。
  捕捉した値は `<:name` で読む（裸の `:name` ではない）。
  動的なスロット名は未対応。

```sprs
@attach(compute(), <:result);
@println(<:result);
```

* `@label_is(value, expected)`: `value` が、名前が `expected`（Atom: `:name` または `:"{ident}-…"`）と一致するラベルのとき `true`。
* `@label_payload(value)`: ラベルのペイロードをクローンする（ラベルでなければ Unit）。
* `@label_name(value)`: ラベル名を `str` として返す（ラベルでなければ `""`）。

```sprs
var v = {:ok, 1};
if @label_is(v, :ok) {
  @println(@label_payload(v));
  @println(@label_name(v));
}
```

Atom/Label 構文、attach スロット、`match` は [ラベルと match](../language/labels-and-match.md) を参照してください。

* `@error(reason)`: 引数ちょうど 1 つで `{:error, reason}` を作る。
* `@is_error(value)`: `value` がエラーラベルのとき `true`。
* `@error_message(value)`: 理由が String のときは String ペイロードをそのまま返す。
  それ以外のペイロードは通常の値フォーマッタで描画する。

`Label(:error, T)`、`?`、未捕捉の `main` エラー、整数オーバーフロー、ゼロ除算は [エラー](../language/errors.md) を参照してください。

**注:** `@cast` マクロは通常の int 型より速いです。
i8 と u8 の llvm 型を直接使うからです。

例:

```sprs
var i = 0; # default is i64
while i < 5 {
  @println(i); ## this is too slow for embedded and system programming environment, because it use dynamic type checking.
 i = i + 1;
}
```

`@cast` マクロを使うと

```sprs
var i = @cast(0, i8); # i is i8 type
while i < @cast(5, i8) {
 @println(i); ## this is faster for embedded system, because it use i8 llvm type directly.
i = i + @cast(1, i8);
}
```

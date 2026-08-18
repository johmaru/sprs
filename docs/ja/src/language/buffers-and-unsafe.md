# Buffer と unsafe

## Buffer

`new(n)` は `n` バイトのゼロ初期化 Buffer を割り当てます（負数 → 無効ハンドル。
`0` は有効な空 Buffer です）。
バイトは `0..=255` の Integer です。
添字糖衣 `buf[i]` は `@bufGet` / `@bufSet` と同様に読み書きします。
書き込みは下位 8 ビットへ切り詰めます。
範囲外の `@bufGet` / `buf[i]` 読み取りは `Unit` 番兵を返します
（リスト添字と同じ規約）。
範囲外の書き込みは何もしません。

`destroy(x)` はヒープ値を明示的に解放し、束縛を `Unit` にします（二重の `destroy` は何もしません）。
`exist(x)` は `x` が生きている Buffer のときだけ `true` です。
スコープを抜けると生きている Buffer は依然として自動 `__drop` されるため、明示的な `destroy` は任意です。

```sprs
var a = new(4);
@bufSet(a, 0, 10);
a[1] = 20;
@println(@bufLen(a));           # 4
@println(a[0] + @bufGet(a, 1)); # 30
@println(exist(a));             # true
destroy(a);
@println(exist(a));             # false
```

## Buffer、destroy、exist

Buffer は他のヒープ値と同じ自動 drop 経路に参加します。
`destroy` なしでスコープを抜けても、生きている Buffer は解放されます。
明示的な寿命切断が必要なときは `destroy` / `defer destroy(...)` を使ってください。
`exist` が報告するのは Buffer の生存だけです。

ムーブセマンティクスと自動 drop は [メモリ管理](../reference/memory-management.md) を参照してください。

## unsafe、RawPtr、defer

`@raw` / `@free` は `unsafe { ... }` の内側でのみ許可されます（入れ子は深さカウンタを増やします）。
`@raw(buf)` は Buffer のバイト割り当てを RawPtr（素のアドレス）へムーブします。
`@raw` のあと、元の束縛は `Unit` になるため、その束縛への後続の自動 drop / `destroy` は何もしません。
呼び出し側がアドレスを所有し、`@free` しなければなりません。
空 / 非 Buffer / 古い入力は null RawPtr（`0`）になります。
`@free` は null と未知のアドレスを無視します。

`defer <expr>;` は `expr` をキューに入れ、スコープ終了時に **LIFO** でキューを実行します。
自動変数 drop の **前** です（`return` 時も含みます）。

```sprs
fn demo() {
  var a = new(1);
  defer destroy(a);   # runs at scope exit before auto-drop
  @bufSet(a, 0, 1);

  var b = new(2);
  defer destroy(b);
  unsafe {
    var p = @raw(b);  # b becomes Unit; deferred destroy(b) is then a no-op
    @free(p);
  }
}
```

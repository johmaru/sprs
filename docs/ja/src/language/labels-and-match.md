# ラベルと match

## ラベル（タグ付き値）

ラベルは値にタグを付ける中核機能であり、エラー専用の型ではありません。
表面型 `Label` は payload なし atom と payload 付きラベルの両方を覆い、単一のランタイムタグではありません。
payload なし `:name` はランタイム `Tag::Atom`（9）、`{:name, payload}` はランタイム `Tag::Label`（10）です。
型位置の `label` / `atom` は `SPRS-SEM-011` で拒否します。`Label`、`:name`、または `Label(:name, T)` と書いてください。
閉じたメンバーは snake_case です（`:Color.red`。`:Color.Red` ではない）。
ラベルは常に名前とペイロード 1 つを持ちます: `{:name, payload}`。
裸の `:name` はペイロードのない不変 Atom（`Tag::Atom`）です。
モジュール大域の Atom 定数は `label :ready;` で、閉じたラベル集合は `label Color { red, blue }` で宣言します。
集合の member は常に完全修飾の `:Color.red` で書きます。
開いた Atom は `:ready`（またはエクスポートされた定数 `ready`）のままです。
宣言をエクスポートするには `pub label ...` を使います。
そうでなければモジュール局所のままです。
同名の局所変数は、単独の Atom 定数をシャドウします。
`:Color.red` の intern キーは `"Color.red"` であり、`:Color.red == :red` は偽です。
未知または非公開の `:Foo.bar` は `SPRS-SEM-004` です。
閉じたラベル集合の宣言は `label Color { red, blue }` だけです。
空集合は構文エラーです。
旧い `enum Color { Red }` と `label :Color{:red}` は拒否されます。
`enum` は通常の識別子です。

```sprs
pub label :ready;                     # exported Atom constant
label Color { red, blue }             # closed label set
label :local_atom;                    # module-local Atom constant

var success_label = :ok;              # Atom
var labeled_value = {:ok, 42};        # Label with payload
var color = :Color.red;               # intern key "Color.red"
@println(ready == :ready);            # true
@println(color == :Color.red);        # true
@println(color == :red);              # false

var item_index = 10;
var dynamic_label = {:"{item_index}-item", 42};   # name becomes "10-item"

if @label_is(dynamic_label, :"{item_index}-item") {
  @println(@label_payload(dynamic_label));  # 42
  @println(@label_name(dynamic_label));     # "10-item"
}

fn wrap(value_input >> i64) >> Label {
  var item_index = value_input;
  return {:"{item_index}", value_input};
}
fn wrap_named(value_input >> i64) >> Label(:ok, i64) {
  return {:ok, value_input};
}
fn take(label_value >> Label) >> Label {
  return label_value;
}

@attach(wrap_named(7), <:item);   # capture into a local slot
@println(<:item);                 # {:ok, 7}
```

注意:

- 動的テンプレートは `{}`、`{expr}`、入れ子の波括弧を拒否します。
  `{ident}` だけを使ってください。
- `@attach(expr, <:name)` はクローンした値を関数局所スロット `<:name` に格納します。
  いずれの `@attach` より前に `<:name` を読むとコンパイルエラーです。
- 裸の `:name` は常に Atom であり、attach スロットをシャドウしません。
- `?` が伝播するのは名前が `:error` のラベルだけです。
  `:ok` のような通常ラベルは通常経路を続けます。

## Match

`match` は静的パターンで Atom / Label 値を分岐します。
形は 2 つあります。
**文**（ブロックまたは束縛変数）と **式**（値を生成）です。

文の形:

- **束縛** — `match <Expr> ?(var name) { case PAT => expr break; … }`。
  各アームは式を評価し、それを `name` に格納して match を抜けます。
  束縛は同じブロック内の match のあとでも見えます。
- **束縛なし** — `match <Expr> { case PAT => { stmts } … }`。
  アームは文ブロックです（`if` と同じ形）。

式の形 — `match <Expr> { case PAT => expr … }` は一致したアームの値を生成します（`break` なし）。
値を消費する文脈が必要です（例: `var r = …;`）。
単独の分岐には束縛なしの文を使います。

パターン（v1、静的な名前のみ）:

- `case :name` — 開いた Atom または Label を名前で一致します（ペイロード束縛なし）
- `case :Set.member` — 閉じたラベル集合の member に一致します（完全修飾）
- `case {:name, binder}` — Label のみ。
  ペイロードを `binder` に束縛します（`_` は捨てます）
- `case _` — 何にでも一致し、最後のアームでなければなりません。
  `Match failed` パニックを避ける既定として使います。

```sprs
fn match_label_bind() >> i64 {
  match {:ok, 7} ?(var r) {
    case :ok => 1 break;
    case :error => 0 break;
  }
  return r;
}

fn match_payload_bind() >> i64 {
  match {:ok, 7} ?(var r) {
    case {:ok, x} => x break;
    case :error => 0 break;
  }
  return r;
}

fn match_atom_bind() >> i64 {
  match :ok ?(var r) {
    case :ok => 1 break;
    case :error => 0 break;
  }
  return r;
}

fn match_no_bind_block() >> i64 {
  var flag = 0;
  match :error {
    case :ok => { flag = 100; }
    case :error => { flag = 1; }
  }
  return flag;
}

fn match_expr_example(v >> Label) >> i64 {
  var r = match v {
    case :ok => 1
    case :error => 0
    case _ => -1
  };
  return r;
}

label State { idle, running }

fn match_closed_label_set() >> i64 {
  var r = match :State.idle {
    case :State.idle => 1
    case :State.running => 2
  };
  return r;
}
```

注意:

- 一致しない被検査値は、末尾の `case _` がすべてを捕まえない限り、`Match failed` でパニックします（プロセスは非ゼロで終了します）。
- 閉じたラベル集合の被検査値はコンパイル時に検査されます。
  すべての member を `:Set.member` で書くか、末尾の `case _` です。
  欠落した member は宣言順の完全修飾で列挙されます
  （`non-exhaustive match on State; missing State.running`）。
  短縮の `case :running` は `State.running` をカバーしません。
- `case :"{i}-item"` のような動的な名前パターンはコンパイル時に拒否されます。
  動的な名前には `if` と `@label_is` を使ってください。
- 束縛マーカーは単一トークン `?(` です。
  後置 Try の `?` と衝突しません（`match x? { … }` は依然として Try のあと match を意味します）。

# 関数と制御フロー

## 関数

```sprs
fn add(a, b) {
   return a + b;
}

fn main() {
 result = add(5, 10);
 @println(result);
}
```

関数に `pub` が付いていなければ、それは非公開関数です。
その関数は同一モジュール内で呼び出せます。

パラメータと戻り値の型は `>>` 注釈を使います。
注釈のないパラメータは動的なままです。
注釈付きパラメータは呼び出し側で検査されます（引数個数と型）。

```sprs
fn add(a >> i64, b >> i64) >> i64 {
  return a + b;
}
```

シグネチャでの `ambi` と適用型は [型と束縛](types-and-bindings.md) を参照してください。
関数名、パラメータ、ローカルは `snake_case` です（`SPRS-SEM-025`）。

## ジェネリック関数

関数名の直後に PascalCase の型パラメータを置けます。

```sprs
fn same<T>(left >> T, right >> T) >> T {
  return left;
}

fn main() {
  same<i64>(1, 2);
  same("a", "b");
}
```

明示した型引数が先に、宣言順で束縛されます。残りは実引数型から左から推論します。
期待する戻り値型からは推論しません。すべてのパラメータを束縛できない呼び出しは
`SPRS-TYP-007`（`cannot infer generic type \`T\` in call to \`foo\``）です。
衝突する実引数も `SPRS-TYP-007` です。明示型引数の個数誤りは `SPRS-SEM-011`
（`generic function \`foo\` expects N type argument(s), found M`）です。

具象引数リストごとに 1 回だけコンパイルします（demand-driven な単相化）。
通常の関数経路へ流します。
複数の importer が同じ public なジェネリック callable を要求しても、宣言モジュールがその specialization を 1 つ所有します。
プログラム全体で同一の `FunctionInstanceId` を重複生成しません。
実行時の generic ディスパッチも追加のランタイムタグもありません。

## FunctionBuild

関数契約は `function_build` で宣言し、`fn name use Build` で結び付けます。
解析後は、同じパラメータ、戻り値型、可視性を通常の `fn` ヘッダに書いたのと同等です。
インライン `fn` 構文は変わりません。旧 `@Fb*` と `#define FunctionBuild` はありません。

```sprs
function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
    visibility(pub);
}

fn add use AddBuild {
    return lhs + rhs;
}
```

ディレクティブ（`params` / `return_type` / `visibility` は各々たかだか 1 回）:

- `params(...)` — パラメータ名と `>>` 型。省略すると引数なし。
- `return_type(T)` — 戻り値型。省略すると注釈なし / `Any`。
- `visibility(pub)` または `visibility(private)` — ビルドを使う関数の可視性。省略すると非公開。
- `type_param T;` — PascalCase の型パラメータ。重複、組み込み / 可視構造体 / 閉じたラベル集合との衝突、未宣言 param の参照はコンパイルエラー。ビルド本体では宣言済み PascalCase 名が `Type::Param` になります。
- `when CONDITION { return_type(T); }` — `return_type` は 1 つだけ。ブロック内に `params` / `visibility` / 入れ子 `when` / マクロは置けません。

関数が使えるビルドはちょうど 1 つです。
`fn name use Build { ... }` は、同じヘッダでインラインパラメータ、`>>` 戻り値注釈、`pub fn` を混ぜられません。
FunctionBuild 名は独自の名前空間にあるため、`function_build Foo` と `fn foo use Foo` は共存できます（`fn` 自体は snake_case）。
同じビルドは複数の関数で再利用でき、使う関数より後に宣言しても構いません。

`pub function_build` は **ビルド宣言** の可視性です（別ソースが結び付けられるかどうか）。
これは `visibility(...)` とは独立で、`visibility(...)` はビルドを使う関数の可視性です。

別ファイルで宣言されたビルドを結び付けるには、そのファイルを FunctionBuild ソースとして指定します。
コンパイラは `{source_path}/name.sprs` を読みます。ランタイムの `import` ではありません。
`function_build` はキーワードなので、パッケージ名に使うならエスケープするか別名（`fn_builds`）にします。

```sprs
# fb_contracts.sprs
pkg fb_contracts;

struct Job {
    id >> i64
}

pub function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
    visibility(pub);
}

function_build InternalBuild {
    return_type(str);
}

fn helper_not_imported() {
}
```

```sprs
# consumer.sprs
pkg consumer;

function_build source fb_contracts;

fn add use AddBuild {
    return lhs + rhs;
}
```

ソース規則:

- 消費側から見えるのは `pub function_build` 宣言だけで、修飾なしの名前（`AddBuild`。
  `fb_contracts.AddBuild` ではない）で参照します。
- ファイルに置ける `function_build source` はたかだか 1 つです（`SPRS-SEM-023`）。
- 入れ子の FunctionBuild ソースは許可、循環は禁止です（`SPRS-SEM-024`）。
- ソース内の通常関数は import されず、消費側にもコンパイルされません。
- ソースで宣言した構造体は `params` と `return_type` の名前付き型として使えます。

### 呼び出し契約の solver

呼び出し側は 1 つの resolver（`resolve_generic_call`）を共有します。

1. 明示型引数があれば個数と具象性を検査し、宣言順に先束縛する。
2. 引数個数（不一致は `SPRS-SEM-016`）。
3. 各パラメータパターンと実引数型を左から unify。`Type::Param` は束縛、`Any` は弱く後続の具体型で上書き、具体型同士の衝突は `SPRS-TYP-007`。
4. 宣言した型パラメータはすべて具体型（`Any` のまま禁止）。
5. `when` 条件を評価。
6. 選んだ戻り値型へ代入。期待する戻り値型からはパラメータを束縛しない。

条件: `is` は代入後の正規型互換、`neq` は両辺が具体型のときだけ否定、`and` / `or` / `not` は短絡、オペランドは型（型パラメータを含む）。
成立する `when` が 0 件なら無条件の `return_type`、それもなければ注釈なし / `Any`。
2 件以上成立すると結果型が同じでも `SPRS-TYP-007`（`multiple \`when\` rules matched`）。
条件付き戻り値だけの本体は `Any` で検査し、呼び出し側の型は resolver 結果を使います。
ジェネリックな FunctionBuild はインラインのジェネリック `fn` と同じく、具象引数リストごとに 1 つの特殊化関数へ単相化されます。

```sprs
function_build Identity {
    type_param T;
    params(value >> T);
    return_type(T);
    visibility(pub);
}

fn identity use Identity {
    return value;
}

fn main() {
  identity<i64>(42);
  identity("id");
}
```

## 制御フロー

```sprs
if x > 5 {
  @println("x is greater than 5");
} else {
 @println("x is 5 or less");
}

while x < 10 {
 println(x);
 i++;
}
```

`else` は省略できます。
`elseif` キーワードはありません。
条件を連鎖するには、入れ子の `if` か連続する `if` 文を使います。
`tests/src/control_flow.sprs` の `if_elseif_chain` がその例です。

```sprs
fn if_elseif_chain() >> i64 {
    var x = 15;
    if x < 10 {
        return 1;
    }
    if x < 20 {
        return 2;
    }
    if x < 30 {
        return 3;
    }
    return 0;
}
```

`return` は現在の関数を抜けます。

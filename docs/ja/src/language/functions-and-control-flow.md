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
検査では `int` と `i64` は同じ型として扱います。
`fp` と `fp64` も同様です。

```sprs
fn add(a >> int, b >> int) >> int {
  return a + b;
}
```

シグネチャでの `ambi` と適用型は [型と束縛](types-and-bindings.md) を参照してください。

## FunctionBuild

関数契約は、本体とは別に `function_build` で宣言できます。
関数は `use` でビルドを 1 つ結び付けます。
意味解析のあとでは、同じパラメータ、戻り値型、可視性を通常の `fn` ヘッダに書いたのと同等です。
既存のインライン `fn` 構文は変わりません。

```sprs
function_build AddBuild {
    @FbArgs(lhs >> i64, rhs >> i64);
    @FbRetTy(i64);
    @FbVisibility(pub);
}

fn add use AddBuild {
    return lhs + rhs;
}
```

Phase 1 のディレクティブ:

- `@FbArgs(...)` はパラメータ名と `>>` 型注釈を指定します。
- `@FbRetTy(T)` は戻り値型を設定します。
- `@FbVisibility(pub)` または `@FbVisibility(private)` は、生成される関数の可視性を設定します。

これらの `@Fb*` 形式はコンパイル時ディレクティブであり、ランタイムマクロではありません。
省略すると、引数なし、戻り値注釈なし、非公開になります。

関数が使えるビルドはちょうど 1 つです。
`fn name use Build { ... }` は、同じヘッダでインラインパラメータ、`>>` 戻り値注釈、`pub fn` を混ぜられません。
FunctionBuild 名は独自の名前空間にあるため、`function_build Foo` と `fn Foo use Foo` は共存できます。
同じビルドは複数の関数で再利用でき、使う関数より後に宣言しても構いません。

`pub function_build` は **ビルド宣言** の可視性です（別ソースが結び付けられるかどうか）。
これは `@FbVisibility` とは独立であり、`@FbVisibility` はビルドを使う関数の可視性です。

別ファイルで宣言されたビルドを結び付けるには、そのファイルを FunctionBuild ソースとして名前で指定します。
コンパイラは `{source_path}/contracts.sprs` を読みます。
ランタイムの `import` は行いません。

```sprs
# contracts.sprs
pkg contracts;

struct Job {
    id >> i64
}

pub function_build AddBuild {
    @FbArgs(lhs >> i64, rhs >> i64);
    @FbRetTy(i64);
    @FbVisibility(pub);
}

function_build InternalBuild {
    @FbRetTy(str);
}

fn helper_not_imported() {
}
```

```sprs
# consumer.sprs
pkg consumer;

#define FunctionBuild contracts

fn add use AddBuild {
    return lhs + rhs;
}
```

ソース規則:

- 消費側から見えるのは `pub function_build` 宣言だけで、修飾なしの名前（`AddBuild`。
  `contracts.AddBuild` ではない）で参照します。
- ファイルに置ける `#define FunctionBuild` は高々 1 つです。
- FunctionBuild ソースの入れ子は許可されます。
  循環は許可されません。
- ソース内の通常関数は、消費側へ import もコンパイルもされません。
- ソースで宣言された構造体は、`@FbArgs` と `@FbRetTy` の名前付き型として使えます。

Phase 2 の形式（`fbtype`、unification、`fbif`）は使えません。

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

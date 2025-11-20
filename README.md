## sprs 言語仕様

この節では、このリポジトリで実装されている簡易言語 **sprs** の仕様について説明します。

### 言語の概要

sprs は、以下のような特徴をもつ、関数ベースの簡易スクリプト言語です。

- `fn` による関数定義
- ブロック `{ ... }` 内でのローカル変数宣言
- 整数リテラル (`42` など)
- 四則演算の一部: 加算 `+`・乗算 `*`
- 等価比較 `==`
- `if ... then { ... } [else { ... }]` による条件分岐
- `return` による関数からの値の返却
- 組み込み関数 `print(...)`
- エントリポイントとなる `main` 関数（存在しなければ最初の関数が呼ばれる）

### 字句仕様（トークン）

字句解析は `src/lexer.rs` で `logos` を用いて実装されています。  
主なトークンは次のとおりです。

- 波括弧: `{` `}` (`LBrace`, `RBrace`)
- 丸括弧: `(` `)` (`LParen`, `RParen`)
- 演算子:
  - `+` (`Plus`)
  - `*` (`Star`)
  - `=` (`Eq`) …… 代入演算子
  - `==` (`EqEq`) …… 等価比較演算子
- 区切り:
  - `;` (`Semi`) …… 文の区切り
  - `,` (`Comma`) …… 引数やパラメータの区切り
- 予約語:
  - `if` (`If`)
  - `then` (`Then`)
  - `else` (`Else`)
  - `fn` (`Function`)
  - `return` (`Return`)
- 識別子:
  - 正規表現 `[A-Za-z_][A-Za-z0-9_]*` にマッチする文字列 (`Ident(String)`)
- 数値リテラル:
  - 正規表現 `[0-9]+` にマッチする 10 進整数 (`Num(i64)`)
- 空白類（スペース・タブ・改行など）はトークンとしては無視されます。

### 構文仕様

構文は LALRPOP (`src/grammar.lalrpop`) で定義され、パーサは `StartParser` として生成されています。  
トップレベルは `Vec<Item>`（複数の関数定義やグローバル変数）です。

#### プログラム構成

- プログラム: 1 個以上の `Item` の列
- `Item` には
  - 関数定義
  - 変数宣言（`VarItem` として AST に乗るが、現状は主に関数内の利用を想定）
    が含まれます。

#### 関数定義

関数定義は次のように書きます:

```sprs
fn 関数名(パラメータ, ...) {
    // 関数本体（文の列）
}
```

- 具体例:

  ```sprs
  fn add(a, b) {
      result = a + b;
      return result;
  }
  ```

- 文法上:
  - `fn` キーワード (`Function`)
  - 関数名（`Ident`）
  - `(` パラメータリスト `)`
    - パラメータリストは 0 個以上の識別子 (`a`, `b` など) を `,` で区切ったもの
  - 本体は `{` と `}` で囲まれたブロック（文の列）

戻り値の型は言語仕様としては明示されません（型は `sema_builder` がヒントを推論しています）。  
`return` が実行された時点で、その値が関数の戻り値となります。`return;` の場合は「値なし」として `Unit` とみなされます。

#### 文（statement）

`Stmt` は次の種類があります (`src/ast.rs` 参照)。

- 変数宣言・代入文

  ```sprs
  x = 5;
  ```

  - 構文: `Ident '=' Expr ';'`
  - AST: `Stmt::Var(VarDecl { ident, expr })`

- 式文

  ```sprs
  expr;
  ```

  例:

  ```sprs
  print(x);
  ```

  - 構文: `Expr ';'`
  - AST: `Stmt::Expr(Expr)`

- `if` 文

  ```sprs
  if 条件式 then {
      // then ブロック
  } else {
      // else ブロック
  }
  ```

  `else` は省略可能です:

  ```sprs
  if 条件式 then {
      // then ブロック
  }
  ```

  - 構文:
    - `if Expr then Block else Block`
    - `if Expr then Block`
  - AST: `Stmt::If { cond, then_blk, else_blk: Option<Vec<Stmt>> }`

- `return` 文

  ```sprs
  return expr;
  ```

  または値なし:

  ```sprs
  return;
  ```

  - 構文:
    - `return Expr ';'`
    - `return ';'`
  - AST: `Stmt::Return(Option<Expr>)`

ブロック `Block` は `{` と `}` で囲まれた文の列 `Vec<Stmt>` です。

#### 式（expression）

`Expr` の定義は `src/ast.rs` の `Expr` enum と `grammar.lalrpop` の `Expr` まわりに対応します。

- 数値リテラル

  ```sprs
  123
  ```

  - AST: `Expr::Number(i64)`

- 変数参照

  ```sprs
  x
  ```

  - AST: `Expr::Var(String)`

- 関数呼び出し

  ```sprs
  func(arg1, arg2)
  ```

  - 構文: `Ident '(' ArgList ')'`
  - `ArgList` は 0 個以上の式を `,` で区切ったもの
  - AST: `Expr::Call(String, Vec<Expr>, Option<Type>)`  
    （第 3 引数の `Option<Type>` は現状 `None` が入ります）

- 加算 `+`

  ```sprs
  lhs + rhs
  ```

  - 左再帰 `Add = Add '+' Mul | Mul` により左結合
  - AST: `Expr::Add(Box<Expr>, Box<Expr>)`

- 乗算 `*`

  ```sprs
  lhs * rhs
  ```

  - 左再帰 `Mul = Mul '*' Factor | Factor` により左結合
  - AST: `Expr::Mul(Box<Expr>, Box<Expr>)`
  - 構文上、`*` は `+` より優先度が高い（`Add` が `Mul` を下位にもつため）

- 等価比較 `==`

  ```sprs
  lhs == rhs
  ```

  - AST: `Expr::Eq(Box<Expr>, Box<Expr>)`
  - `Equality = Equality '==' Add | Add` により、`==` は `+`・`*` よりも結合順序が外側になります。

- 括弧付き式

  ```sprs
  (expr)
  ```

  - AST 上は中身の式そのもの（`Factor = '(' Expr ')' => e`）

#### 優先順位と結合規則

構文定義から、演算子の優先順位と結合規則は次のようになります（上ほど強い）。

1. 関数呼び出し / 変数 / 数値 / 括弧（`Factor`）
2. 乗算 `*`（左結合）
3. 加算 `+`（左結合）
4. 等価比較 `==`（左結合）

### 実行時仕様（評価モデル）

実行は `src/executer.rs` に定義されています。

#### 値の種類

評価結果は `Value` 型で表現されます。

- `Value::Int(i64)` …… 整数
- `Value::Bool(bool)` …… 真偽値
- `Value::Str(String)` …… 文字列（現状の構文には文字列リテラルはありませんが、内部的には型が用意されています）
- `Value::Unit` …… 値なし（`()` と表示）
- `Value::Return(Box<Value>)` …… 関数からの「戻り値」を伝播するための内部用ラッパ

#### 演算の意味

- `+`（加算）

  - 両辺が `Value::Int` のときだけ `Int(a + b)` になります。
  - それ以外の型の組み合わせは、デモ実装として `Unit` を返します。

- `*`（乗算）

  - 両辺が `Value::Int` のときだけ `Int(a * b)` になります。
  - それ以外の型の組み合わせは、`Unit` を返します。

- `==`（等価比較）

  - `Value` の `PartialEq` 実装に依存して比較され、`Value::Bool` を返します。
  - 型が異なるときは通常 `false` になります。

#### `if` の評価

- `if` 文の条件式は `Value` に評価されたあと、次のように真偽判定されます:

  - `Value::Bool(b)` の場合: そのまま `b`
  - `Value::Int(n)` の場合: `n != 0` が真
  - それ以外の値: 偽

- 条件が真なら `then` ブロックを、偽なら `else` ブロック（あれば）を評価します。
- ブロック内で `return` が発生し、`Value::Return` が返ってきた場合、その値は関数の外側にそのまま伝播します（早期リターン）。

#### 関数呼び出し

- ユーザー定義関数

  - `fn name(p1, p2, ...) { ... }` で定義された関数は、呼び出し時に引数が左から順に評価され、ローカルスコープにパラメータ名でバインドされます。
  - 関数本体のブロックを `execute_block` で順に評価します。
  - ブロックの評価中に
    - `return expr;` で `expr` が評価され、その値が `Value::Return(val)` として返される
    - `return;` で `Value::Return(Unit)` が返される
  - 関数呼び出しの外側では、`Value::Return(val)` から中身の `val` を取り出して、呼び出し式全体の結果とします。

- 組み込み関数 `print`

  - 関数名が `"print"` のときは特別扱いです。
  - 引数は左から順に評価され、それぞれ `println!` で表示されます。
  - 戻り値は常に `Value::Unit` です。

#### 変数スコープ

- 関数ごとに 1 つのローカルスコープ（`HashMap<String, Value>`）が用意されます。
- 変数宣言 `x = expr;` によって `scope["x"] = value` が登録され、その後の同一関数内で参照できます。
- `if` ブロックの中で宣言された変数も、実装上は同じスコープを共有しています（ブロックスコープの区別はありません）。

### 型推論と補助情報

`src/sema_builder.rs` では、次のような簡易な型推論が行われます。

- 関数ごとのシグネチャ `ItemSig` を収集し、戻り値型をざっくり推定
  - `return` 文や `if` ブロック内の `return` などから、`Int` や `Bool` を推論
  - 推論できない場合は `Type::Any`
- 変数宣言ごとに `VarInfo` を作り、`Type` をヒントとして保持
  - 数値リテラルは `Int`
  - `Eq` は `Bool`
  - 二項演算 `+`, `*` は両辺が `Int` なら `Int`
  - `Var` や複雑な式は必要に応じて `Any`

これらは現在、主にデバッグ出力として利用されています。

### 実行例

`src/main.rs` には、sprs 言語の一例として次のコードが含まれています。

```sprs
fn test() {
    a = 5;
    b = 10;

    if a == 5 then {
        return a;
    }

    return b;
}

fn main() {
   x = test();
   print(x);
}
```

このプログラムの挙動:

1. `main` 関数がエントリポイントとして選ばれます。
2. `main` 内で `x = test();` が実行されます。
3. `test` 関数では
   - `a = 5;`
   - `b = 10;`
   - `if a == 5 then { return a; }`  
     条件 `a == 5` は真なので、`return a;` により `5` が返されます。
4. `x` には `5` が代入されます。
5. `print(x);` により、標準出力に `5` が表示されます。

### 実行方法の概要

例:

```bash
cargo run
```

`src/main.rs` に埋め込まれているサンプルコードがパース・実行され、`print` などの結果が出力されます。

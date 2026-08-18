# エラー

コンパイル時コード（`SPRS-SYN-001`、`--error-format`）は [コンパイルエラー](../reference/compiler-errors.md) で説明します。
この章ではランタイムのエラーラベルを扱います。

## エラーラベル

エラーは通常のラベルであり、専用のランタイム値ではありません。
シグネチャには `Label(:error, Any)`（または広い `Label`）と書きます。`@error(reason)` は引数ちょうど 1 つで `{:error, reason}` を作ります。`err` 型別名はありません。

同じ値は、通常のラベルリテラルとして直接作れます。
エラー名とペイロード型を関数シグネチャの一部にするときは `Label(:error, T)` を使います。

```sprs
fn make_error_label() >> Label(:error, str) {
  return {:error, "file not found"};
}

fn main() {
  var error_label_value = make_error_label();
  @println(@is_error(error_label_value));         # true
  @println(@error_message(error_label_value));    # file not found
}
```

`@error(reason)` は、同じラベル規約の短縮形です。

```sprs
fn make_error() >> Label(:error, Any) {
  return @error("file not found");
}

fn show_error() {
  var error_value = make_error();
  @println(@is_error(error_value));         # true
  @println(@error_message(error_value));    # file not found
  @println(@error_message(@error(:enoent))); # :enoent
}
```

`@error_message` は、理由が String のときは String ペイロードをそのまま返します。
それ以外のペイロードは、通常の値フォーマッタで描画されます。
削除された `@error_code` マクロと、旧来の `Tag::Error` ABI はもう使えません。payload なし atom はランタイム `Tag::Atom = 9`、payload 付きラベルは `Tag::Label = 10` です。

エラーラベルが処理されずに `main` 境界へ達すると、Sprs は `Uncaught error in main` を表示して終了します。
既知のランタイム制限として、その後のスレッドローカルスロット掃除が TLS destruction 警告を出すことがあります。
ラベルペイロードの掃除が、破棄開始後の同じスレッドローカルスロット表へ再入するからです。
この警告はプロセス終了中、未捕捉エラーメッセージのあとに起き、エラーラベルの結果は変えません。

`?` が伝播するのは名前が `:error` のラベルだけです。
`:ok` のような通常ラベルは通常経路を続けます。

## 整数オーバーフロー

整数の `+`、`-`、`*` は整数型の符号とビット幅に対して検査され、`/` と `%` はさらに演算前に符号付き最小値 / `-1` の組み合わせを検査します。
成功時の結果は通常の整数値です。
オーバーフロー時は完全なラベル `{:error, :overflow}` が返ります。
`@is_error`、`@label_payload`、`@error_message`、`?` は、他のエラーラベルと同じようにこれに働きます。

```sprs
fn propagate_overflow() >> i64 {
  var value = (9223372036854775807 + 1)?;
  return value;
}

fn inspect_overflow() {
  var value = 9223372036854775807 + 1;
  @println(@is_error(value));       # true
  @println(@label_payload(value));  # :overflow
  @println(@error_message(value));  # :overflow
}
```

異なる整数タグは以前どおり既定の `i64` へ昇格します。
`++` と `--` はこの契約の対象外です。
ゼロ除算は既存の `{:error, "Division by zero"}` ラベルのままです。

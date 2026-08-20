# 演算子

- 算術: `+`、`-`、`*`、`/`、`%`
- 比較: `==`、`!=`、`<`、`>`、`<=`、`>=`
- インクリメント / デクリメント: `++`、`--`（後置のみ）
- Range 生成: `..`（例: `1..10`）
- 添字: `list[index]` / `buf[index]`（Buffer はバイトの get/set を使う）。`List(T)[i] = v` は `v` を `T` に対して検査する。
- 単項マイナス: `-x`（`Expr::Neg`）。
- 文字列連結: `str + str` は `__string_concat` を呼び出します。
  整数加算ではありません。
- ビット演算のトークンはありません。
  シフトと論理否定はマクロです: [`@lshift`](../reference/built-in-macros.md)、[`@rshift`](../reference/built-in-macros.md)、[`@not`](../reference/built-in-macros.md)。

整数の `+`、`-`、`*` のオーバーフロー、および `/` / `%` のオーバーフロー検査は [エラー](errors.md) で説明します。
ゼロ除算は既存の `{:error, "Division by zero"}` ラベルのままです。
`++` と `--` は整数オーバーフロー契約の対象外です。
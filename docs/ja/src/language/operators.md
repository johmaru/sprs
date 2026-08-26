# 演算子

- 算術: `+`、`-`、`*`、`/`、`%`
- 比較: `==`、`!=`、`<`、`>`、`<=`、`>=`
- インクリメント / デクリメント: `++`、`--`（後置のみ）
- Range 生成: `..`（例: `1..10`）
- 添字: `list[index]` / `buf[index]`（Buffer はバイトの get/set を使う）。`List(T)[i] = v` は `v` を `T` に対して検査する。
- 単項マイナス: `-x`（`Expr::Neg`）。
- 前置の間接参照: `*p`（`Expr::Deref`）。優先順位は `-x` と同じ。`*p` は pointee を読む。`*p = value` は代入先（`Stmt::DerefAssign`）。入れ子の `**pp` はできる。
- ポインタ加算: `Ptr(T) + offset` は要素単位（stride は `{ tag, data }` 1 スロット）。`offset` は `usize` または非負の整数リテラル。オーバーフローは `Pointer arithmetic overflow` で panic する。`integer + Ptr(T)`、負リテラル、`Ptr(T)` の `-` `*` `/` `%` は `SPRS-TYP-001`。
- 文字列連結: `str + str` は `__string_concat` を呼び出します。
  整数加算ではありません。
- ビット演算のトークンはありません。
  シフトと論理否定はマクロです: [`@lshift`](../reference/built-in-macros.md)、[`@rshift`](../reference/built-in-macros.md)、[`@not`](../reference/built-in-macros.md)。

整数の `+`、`-`、`*` のオーバーフロー、および `/` / `%` のオーバーフロー検査は [エラー](errors.md) で説明します。
ゼロ除算は既存の `{:error, "Division by zero"}` ラベルのままです。
`++` と `--` は整数オーバーフロー契約の対象外です。
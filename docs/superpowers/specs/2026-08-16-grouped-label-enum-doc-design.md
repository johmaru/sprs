> **正本の変更:** 利用者向け文書の正本は `docs/src/` です。`src/main.rs` を編集して `cargo rdme` を実行する手順は失効しています。このファイルは履歴として残します。

# grouped labelによるenum互換構文の説明

## 目的

READMEの`enum`節には通常の`enum`宣言だけがあり、grouped labelによる互換構文が同じコンパイル時フレームを作ることを確認できない。

この不足を補い、読者が`label :Color{:red, :blue}`から`Color.red`を参照できることを、説明と実行例で確認できるようにする。

## 変更

`src/main.rs`のcrate docにある`enum`節へ、grouped label宣言の説明を追加する。

説明では、次の事実を区別して記載する。

- 通常の`enum Color { Red }`は`Color.Red`を生成する。
- grouped labelの`label :Color{:red}`は同じ種類のコンパイル時フレームを作り、`Color.red`を生成する。
- どちらの値も実行時には枠付きintern keyを持つAtomである。
- `pub label`を使用すると、宣言した名前空間を別モジュールへ公開できる。

同じ節へ、次の構文を含む短い実行例を追加する。

```sprs
pub label :Color{:red, :blue}

fn main() {
  @println(Color.red);
}
```

READMEは直接編集せず、`cargo rdme --force`でcrate docから生成する。

## 検証

`cargo rdme --check`でREADMEと生成元の一致を確認する。

既存の変更を含むため、`cargo test`と`cargo run -- run --dest tests`も実行してからコミットする。

## 公開

既存の整数オーバーフロー対応と今回の文書変更をコミットし、現在の`dev`ブランチを`origin/dev`へpushする。

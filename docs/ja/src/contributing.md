# 貢献

この言語の開発環境は WSL2(Ubuntu) + VSCode を推奨します。

1. Rust と WSL2(Ubuntu) をインストールします。
2. ```bash
sudo apt update && sudo apt install -y lsb-release wget software-properties-common gnupg
```
3. ```bash
wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && sudo ./llvm.sh 22 all
```
4. ```bash
sudo update-alternatives --install /usr/bin/clang clang /usr/bin/clang-22 100 && sudo update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-22 100 && sudo update-alternatives --install /usr/bin/llvm-config llvm-config /usr/bin/llvm-config-22 100 && sudo update-alternatives --install /usr/bin/llvm-as llvm-as /usr/bin/llvm-as-22 100 && sudo update-alternatives --install /usr/bin/llc llc /usr/bin/llc-22 100
```
5. ```bash
sudo apt-get install zlib1g-dev libzstd-dev && sudo apt-get install libncurses5-dev libxml2-dev
```
6. このリポジトリをクローンし、VSCode で開きます。
7. VSCode の Rust 拡張をインストールします。
8. `cargo build` と `cargo run` でプロジェクトをビルドして実行します。

## ローカルでのドキュメント

リポジトリルートから、両ブックの Markdown ファイル集合が一致することを確認し、英語版の次に日本語版をビルドします。

```bash
diff \
  <(find docs/en/src -type f -name '*.md' -printf '%P\n' | sort) \
  <(find docs/ja/src -type f -name '*.md' -printf '%P\n' | sort)

mdbook build docs/en
mdbook build docs/ja
```

両ブックをまとめて配信します。

```bash
python3 -m http.server 3000 --directory docs/book
```

英語版は `http://127.0.0.1:3000/`、日本語版は `http://127.0.0.1:3000/ja/` を開きます。
生成された HTML は `docs/book/` に書き出され、コミットしません。

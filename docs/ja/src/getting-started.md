# 入門

## コンパイラコマンド

引数なしでは、コンパイラは stderr に `Usage: sprs help --all` を書き、`"invalid command"` で失敗します。

```bash
# To build the project
sprs build

# To run the project
sprs run
```

`sprs build`、`sprs run`、`sprs debug` は同じオプションを受け付けます。
`build` はコンパイルしてリンクします。
`run` は同じ処理をしたあとプログラムを実行します（`ExecuteMode::Run` のみ）。
`debug` は `build` と同様にコンパイルしてリンクし、プログラムは実行しません。
`sprs help --all` は `debug` を列挙しませんが、コマンド自体は受け付けます。

`sprs help` は短いヘルプを表示します。
`sprs help --all` は完全なヘルプを表示します。
それ以外の help 引数は stderr に `Unknown help argument. Use --all.` を書き、失敗します。

短いヘルプ（`sprs help`）:

```text
Sprs Compiler Help:
Usage: sprs <source_file.sprs> [options]
Options:
---This Section is 'Command' Section---
  init <?args>  Initialize the project
  help          Show this help message
  version       Show compiler version
---This Section is 'Option' Section---
  --name <name>  Set the name of the project
  --all           Show all available commands and options
```

完全なヘルプ（`sprs help --all`）:

```text
Sprs Compiler Full Help:
Usage: sprs <source_file.sprs> [options]
Options:
---This Section is 'Command' Section---
  init <?args>  Initialize the project
  build         Build the project
  run           Run the project
  help          Show this help message
  version       Show compiler version
---This Section is 'Option' Section---
  --name <name>  Set the name of the project
  --all           Show all available commands and options

Sprs is the Sprs compiler, a simple compiler for the Sprs programming language.
For more information, visit the official documentation.
```

`sprs version` は `sprs version: ` のあとにコンパイラ crate のバージョン（`CARGO_PKG_VERSION`）を表示します。

それ以外のコマンド名は stderr に `Unknown command: <name>` を書き、失敗します。

## オプション

`sprs build|run|debug [--dest <path>] [--error-format <human|json|json-pretty>]`:

- `--dest` はプロジェクトの基準ディレクトリを設定します。
  省略時の基準は `.` です。
- `--error-format` は診断の描画を選びます。
  許可される値は `human`、`json`、`json-pretty` です。
- 未知の引数は stderr に `Unknown argument: <arg>` を書き、失敗します。
- 値なしの `--dest` は `Usage: sprs <command> --dest <path>` を書き出します。
- 値なしの `--error-format` は `Usage: sprs <command> --error-format <json|json-pretty|human>` を書き出します。

`sprs init` のオプションは「プロジェクト初期化」で説明します。

## プロジェクト初期化

```bash
sprs init --name <project_name>
```

このコマンドは、既定の `sprs.toml` 設定ファイルと、サンプルの `src/main.sprs` ソースファイルを作成します。

`sprs init [--name <name>] [--force]`:

- `--name` がない場合、コンパイラは `Initializing project without arguments.` を表示し、`sprs_project` を使います。
- 値なしの `--name` は `Usage: sprs init --name <project_name>` を書き、失敗します。
- それ以外の引数は `Usage: sprs init --name <project_name> [--force]` を書き、失敗します。
- 名前は `[A-Za-z0-9_-]+` に一致する必要があります。
- `sprs.toml` または `src/main.sprs` が既にある場合、`--force` がなければ init は上書きを拒否します。
- 生成される `src/main.sprs` は次のとおりです。

```sprs
fn main() {
    @println("Hello, Sprs!");
}
```

生成される `sprs.toml` のキーは [プロジェクト設定](reference/project-config.md) で説明します。

## エラー形式

`--error-format` は `sprs.toml` の `error_format` より優先されます。
どちらも設定されていなければ、形式は `human` です。
JSON と `json-pretty` の診断は stdout に書き、`human` の診断は stderr に書きます。
コンパイル失敗時の終了ステータスは `1` です。

診断スキーマとコードは [コンパイルエラー](reference/compiler-errors.md) を参照してください。

## ビルド成果物

コンパイルが成功すると `{out_dir}/{module}.ll`、`{out_dir}/{module}.o`、`{out_dir}/runtime.rs`、`{out_dir}/libruntime.a` を書き出します。
実行ファイルは Linux では `{out_dir}/{name}`、コンパイル対象が Windows のときは `{out_dir}/{name}.exe` です。
リンクは `clang` に `-lm -ldl -lpthread` を付けて行います。

ホスト OS と `#define` 由来のターゲット OS が異なる場合、コンパイラは警告を表示します。
その場合 `sprs run` は実行をスキップします（`[Skip] Target OS (...) differs from host OS (...). Skipping execution.`）。

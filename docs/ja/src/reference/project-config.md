# プロジェクト設定

プロジェクト設定は、プロジェクト基準ディレクトリ（`--dest`、省略時は `.`）の `sprs.toml` にあります。
読み取るキーは次だけです。

| Key | Type | Default when missing |
|-----|------|----------------------|
| `name` | string | `sprs_project`（`validate_name` を通る必要がある: `[A-Za-z0-9_-]+`） |
| `version` | string | ファイルに記録される。ビルド経路はこのキーを読まない |
| `src_dir` | string | `src`（相対。`..` は拒否される） |
| `out_dir` | string | `build`（相対。`..` は拒否される） |
| `error_format` | optional string | 未設定。CLI も `--error-format` を省略した場合、診断は `human` を使う。許可される値: `human`、`json`、`json-pretty` |

`sprs init` は `src_dir = "src"` と `out_dir = "out"` を書き出します。
それらの生成値は、上表の欠落時既定ではありません。

`sprs.toml` が無い、または TOML 解析に失敗した場合、コンパイラは stderr にメッセージ（`Failed to read sprs.toml: ...` または `Failed to parse sprs.toml: ...`）を書き、設定なしで続行し、表の既定値を使います。

コンパイルの入口は `{base}/{src_dir}/main.sprs` です。

`--dest`、`--error-format`、init は [入門](../getting-started.md) を参照してください。

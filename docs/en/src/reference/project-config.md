# Project Config

Project settings live in `sprs.toml` at the project base directory (`--dest`, or `.` when `--dest` is omitted). Only these keys are read:

| Key | Type | Default when missing |
|-----|------|----------------------|
| `name` | string | `sprs_project` (must pass `validate_name`: `[A-Za-z0-9_-]+`) |
| `version` | string | Recorded in the file; the build path does not read this key |
| `src_dir` | string | `src` (relative; `..` is rejected) |
| `out_dir` | string | `build` (relative; `..` is rejected) |
| `error_format` | optional string | Unset. If the CLI also omits `--error-format`, diagnostics use `human`. Allowed values: `human`, `json`, `json-pretty` |

`sprs init` writes `src_dir = "src"` and `out_dir = "out"`. Those generated values are not the missing-key defaults above.

If `sprs.toml` is missing or TOML parsing fails, the compiler writes a message to stderr (`Failed to read sprs.toml: ...` or `Failed to parse sprs.toml: ...`) and continues with no config, using the defaults in the table.

The compile entry point is `{base}/{src_dir}/main.sprs`.

See [Getting Started](../getting-started.md) for `--dest`, `--error-format`, and init.

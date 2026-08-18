# Getting Started

## Compiler commands

With no arguments, the compiler writes `Usage: sprs help --all` to stderr and fails with `"invalid command"`.

```bash
# To build the project
sprs build

# To run the project
sprs run
```

`sprs build`, `sprs run`, and `sprs debug` accept the same options. `build` compiles and links. `run` does the same and then executes the program (`ExecuteMode::Run` only). `debug` compiles and links like `build` and does not run the program. `sprs help --all` does not list `debug`, but the command is accepted.

`sprs help` prints the short help. `sprs help --all` prints the full help. Any other help argument writes `Unknown help argument. Use --all.` to stderr and fails.

Short help (`sprs help`):

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

Full help (`sprs help --all`):

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

`sprs version` prints `sprs version: ` followed by the compiler crate version (`CARGO_PKG_VERSION`).

Any other command name writes `Unknown command: <name>` to stderr and fails.

## Options

`sprs build|run|debug [--dest <path>] [--error-format <human|json|json-pretty>]`:

- `--dest` sets the project base directory. When omitted, the base is `.`.
- `--error-format` selects diagnostic rendering. Allowed values are `human`, `json`, and `json-pretty`.
- An unknown argument writes `Unknown argument: <arg>` to stderr and fails.
- `--dest` without a value writes `Usage: sprs <command> --dest <path>`.
- `--error-format` without a value writes `Usage: sprs <command> --error-format <json|json-pretty|human>`.

`sprs init` options are described under Project initialization.

## Project initialization

```bash
sprs init --name <project_name>
```

This command creates a default `sprs.toml` configuration file and a sample `src/main.sprs` source file.

`sprs init [--name <name>] [--force]`:

- With no `--name`, the compiler prints `Initializing project without arguments.` and uses `sprs_project`.
- `--name` without a value writes `Usage: sprs init --name <project_name>` and fails.
- Any other argument writes `Usage: sprs init --name <project_name> [--force]` and fails.
- The name must match `[A-Za-z0-9_-]+`.
- If `sprs.toml` or `src/main.sprs` already exists, init refuses to overwrite them unless `--force` is given.
- Generated `src/main.sprs` is:

```sprs
fn main() {
    @println("Hello, Sprs!");
}
```

Keys in the generated `sprs.toml` are documented in [Project Config](reference/project-config.md).

## Error format

`--error-format` wins over `error_format` in `sprs.toml`. If neither is set, the format is `human`. JSON and `json-pretty` diagnostics are written to stdout; `human` diagnostics are written to stderr. A compile failure exits with status `1`.

See [Compiler Errors](reference/compiler-errors.md) for the diagnostic schema and codes.

## Build artifacts

A successful compile writes `{out_dir}/{module}.ll`, `{out_dir}/{module}.o`, `{out_dir}/runtime.rs`, and `{out_dir}/libruntime.a`. The executable is `{out_dir}/{name}` on Linux, or `{out_dir}/{name}.exe` when the compile target is Windows. Linking uses `clang` with `-lm -ldl -lpthread`.

If the host OS and the target OS from `#define` differ, the compiler prints a warning. `sprs run` skips execution in that case (`[Skip] Target OS (...) differs from host OS (...). Skipping execution.`).

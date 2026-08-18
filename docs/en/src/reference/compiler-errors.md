# Compiler Errors

This chapter describes **compile-time** diagnostics. Runtime error labels (`{:error, ...}`, `?`, uncaught `main`) stay in [Errors](../language/errors.md). CLI flags are in [Getting Started](../getting-started.md).

## Codes

Codes use `SPRS-<SYN|TYP|SEM>-NNN` with a three-digit number. Internal failures use JSON `code` `SPRS-INTERNAL`. Human internal output is `internal error: ...` (the `Display` form is `Internal error: ...`).

## Format selection

`--error-format` on `sprs build|run|debug` wins over `error_format` in `sprs.toml`. If neither is set, the format is `human`. Allowed values: `human`, `json`, `json-pretty`. JSON and `json-pretty` reports go to stdout; `human` reports go to stderr. Compile failure exits with status `1`.

## JSON schema

Every JSON object uses these keys:

| Key | Meaning |
|-----|---------|
| `code` | `SPRS-SYN-001` style, or `SPRS-INTERNAL` |
| `category` | `Syntax`, `Semantic`, `Type`, or `Internal` |
| `phase` | always `"compile"` |
| `severity` | always `"error"` |
| `message` | diagnostic text |
| `location` | object: `file`, `line`, `column`, `end_line`, `end_column`, `snippet` |
| `expected` | token names for some parse errors; otherwise `[]` |
| `expected_type` | present on some type errors |
| `actual_type` | present on some type errors |
| `help` | optional help string |

Line and column numbers are 1-based. An Internal error with no location uses `file` `"<unknown>"` and line/column `0`.

## Human format

Human reports use `error[CODE]`, a `--> file:line:column` location, a source snippet, and `expected` / `help` when present. Internal reports use `internal error:` instead of `error[CODE]`.

## Code catalog

The same number can cover several messages. The table lists patterns; a number is not a single meaning.

### Syntax (`SPRS-SYN-…`)

| Code | Message patterns |
|------|------------------|
| SYN-001 | `InvalidToken` |
| SYN-002 | `UnrecognizedToken '{token}'`; `` `{keyword}` is a reserved keyword `` (identifier position; help: `use ^{keyword} if this name is intentional`) |
| SYN-003 | `ExtraToken '{token}'` |
| SYN-004 | `UnrecognizedEOF` |
| SYN-005 | User parse error containing `Invalid assignment target` (message is the parser string) |
| SYN-006 | User parse errors containing `Expected IDENT token`, `Expected MACRO token`, `Expected NUM token`, `Expected FLOAT token`, or `Expected StrLiteral token`; `invalid FunctionBuild directive @{name}`; also other `ParseError::User` messages that are not SYN-005 or SYN-007 |
| SYN-007 | (unused for `@init`; struct init is core `init Type { ... }`) |
| SYN-008 | `unnecessary identifier escape \`^{name}\`` (help: `use {name} instead of ^{name}`) |

### Type (`SPRS-TYP-…`)

| Code | Message patterns |
|------|------------------|
| TYP-001 | `Type mismatch: Function expects pointer type (e.g. str) but got {type} from expression {expr}` |
| TYP-002 | `Type mismatch: Function expects Bool but got {type} from expression {expr}` |
| TYP-003 | `Type mismatch: Function expects Int type but got {type} from expression {expr}` |
| TYP-004 | `Type mismatch: Function expects Float type but got {type} from expression {expr}` |
| TYP-005 | `Type mismatch: Function declares >> {expected} but return expression has {actual}` |
| TYP-006 | `Type mismatch: cannot assign {rhs} to fixed binding `{name}` of type {ty}` |
| TYP-007 | `Type mismatch: argument {n} of `{fn}` expects {ty}, found {actual}`; `Type mismatch in call to `{fn}`: type parameter `{T}` was already resolved to `{ty}`, but the argument has type `{actual}`; `Type mismatch in call to `{fn}`: multiple `when` rules matched`; unresolved type parameter |

### Semantic (`SPRS-SEM-…`)

There is no `SEM-001`, `SEM-012`, or `SEM-014` in the current compiler.

| Code | Message patterns |
|------|------------------|
| SEM-002 | `Undefined variable: {name}`; `Undefined variable in dynamic label name: {name}`; `attach slot '<:{name}' used before @attach` |
| SEM-003 | `Unknown macro: {name}`; `@is_error` / `@error_message` / `@label_payload` / `@label_name` expect exactly 1 argument; `@error expects exactly 1 argument: reason`; `@attach expects exactly 2 arguments: expression and label`; `@attach second argument must be a slot such as <:name`; `@label_is expects exactly 2 arguments: value and label`; `@label_is second argument must be an atom such as :name or :"{i}-item"`; `dynamic label name part `{part}` has type {ty}; only int/bool/str allowed` |
| SEM-004 | `Undefined closed label member: {set.member}`; `Duplicate closed label set: {name}`; `Duplicate label: {name}` |
| SEM-005 | (removed; old `@init` is `Unknown macro: init` / SEM-003) |
| SEM-006 | `Unknown runtime function: {name}` |
| SEM-007 | `Field '{field}' not found in struct '{name}'`; `Undefined struct : {name}` |
| SEM-008 | `@cast second argument must be a type identifier : {expr}` |
| SEM-009 | `Unsupported target type for @cast: {ty}` |
| SEM-010 | `Failed to read module file {path}: {error}` |
| SEM-011 | `Undefined type: {name}`; `` `Self` is only valid in struct field type annotations ``; `unknown type `{legacy}`; use {replacement}` (`int`→`i64`, `list`→`List(T)`, `err`→`Label(:error, Any)`, `atom`/`label`→`Label`, …); `List requires exactly one type argument`; `Label application must be Label or Label(:name, T)` |
| SEM-013 | Macro arity (`list_push expects 2 arguments`, `buf_len expects 1 argument`, `buf_get expects 2 arguments`, `buf_set expects 3 arguments`, `@clone expects 1 argument`, `@move expects 1 argument`, `@move expects a variable argument`, `@cast expects 2 arguments`, `@fcast expects exactly 1 argument`, `@lshift expects 2 arguments (value, shift_amount)`, `@rshift expects 2 arguments (value, shift_amount)`, `@not expects 1 argument`); `@raw` / `@free` require an unsafe block; `Undefined variable: {name}`; `Module '{name}' not found`; `Function '{fn}' not found in module '{module}'`; `Undefined struct : {name}`; `Field '{field}' not found in struct '{name}'`; `Field definition for '{field}' not found in struct '{name}'`; `unknown field \`{field}\` in init {Type}`; `duplicate field \`{field}\` in init {Type}`; `missing required field \`{field}\` in init {Type}` |
| SEM-015 | `Undefined function: {name}`; `` `@raw` requires an unsafe block ``; `` `@free` requires an unsafe block `` |
| SEM-016 | `Argument count mismatch: function `{fn}` expects {n} argument(s), found {m}` |
| SEM-017 | `match patterns must be static :name in v1`; `payload pattern requires Label scrutinee`; `case _ must be the last match arm`; `non-exhaustive match on {set}; missing {Set.member, ...}` |
| SEM-018 | ``undefined FunctionBuild `{name}` `` |
| SEM-019 | ``duplicate FunctionBuild `{name}` `` |
| SEM-020 | `duplicate FunctionBuild directive {name}` (`params` / `return_type` / `visibility`) |
| SEM-022 | ``FunctionBuild `{name}` is private and cannot be used from an external source`` |
| SEM-023 | ``multiple `function_build source` directives in one file`` |
| SEM-025 | `function names must use snake_case` (and other category messages: module/variable/field/macro snake_case, type names PascalCase, label members snake_case) |
| SEM-024 | `circular FunctionBuild source: {a} -> {b} -> ...` |

### Internal

| Code | Message patterns |
|------|------------------|
| SPRS-INTERNAL | Compiler bugs and leftover `From<String>` conversions (`Internal error: ...` / JSON `SPRS-INTERNAL`) |

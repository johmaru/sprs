# Types and Bindings

## Canonical types

Surface types have one spelling. Old aliases (`int`, `fp`, `fp16`/`fp32`/`fp64`, `list`, `range`, `buffer`, `rawptr`, `err`, `atom`, type-position `label`) are `SPRS-SEM-011` with a replacement in `help`.

| Surface | Meaning |
|---------|---------|
| `i8` `u8` `i16` `u16` `i32` `u32` `i64` `u64` | Integer widths. Unannotated integer literals are `i64`. |
| `f16` `f32` `f64` | Float widths. Unannotated float literals are `f64`. `@cast` uses these names. |
| `bool` | Boolean |
| `str` | String |
| `unit` | Unit (`()`) |
| `Any` | Unchecked / dynamic |
| `List(T)` | List. Always one argument (`List(Any)` when the element is unknown). Bare `List` / `List()` is rejected. |
| `Process(T)` | Compile-time process result type. Arity 1. Runtime execution is separate. |
| `Range` | Range |
| `Buffer` | Fixed-size zero-initialized byte array |
| `RawPtr` | Bare address from `@raw(buf)` |
| `Label` | Broad label: payloadless atoms and payload labels |
| `:name` | Exact payloadless atom (`:ready`) |
| `Label(:name, T)` | Exact payload label. First argument must be `:name`. |
| PascalCase name | Struct or closed label set (`Point`, `ConnectionState`) |

`Type::Label` is a surface union. It does not assume a single runtime tag (`tag_discriminant` is `None`). Runtime still uses `Tag::Atom = 9` for payloadless atoms and `Tag::Label = 10` for payload values. Those tags are implementation detail, not type names.

Inferred payloadless value `:ready` is `:ready`. A payload value `{:ok, 7}` is `Label(:ok, i64)` when the name is static, otherwise broad `Label`.

Broad `Label` accepts exact atoms, payload labels, closed label sets, and the runtime-only atom tag. Exact `:name` matches only that name. `Label(:name, T)` compares the name and the payload type. `Label(:ok)` (one argument) and `Atom(...)` are rejected.

`Self` is valid only in struct field types (including `List(Self)`). See [Structs](structs.md).

Type application is compile-time only: `List(i64)`, `Process(str)`, `Label(:ok, i64)`, `Label(:error, Any)`. Unknown constructors (`Result(i64, str)`) and wrong arity (`List(i64, str)`, `Process()`, `Range(i64)`) are `SPRS-SEM-011`. User-defined generic types are not supported.

`List(T)` and `Process(T)` share runtime tags only for `List` (`Tag::List`). `Process(T)` is not a runtime tag yet. Element / result types are compile-time only.

## Typed lists

Unannotated list literals infer an element type:

```sprs
var xs = [1, 2, 3];       # List(i64)
var names = ["a", "b"];   # List(str)
var mixed = [1, "a"];     # List(Any)
var empty = [];           # List(Any)
```

An expected type from `>>`, a return annotation, a call argument, or assignment is used for empty lists and element checks:

```sprs
var xs >> List(i64) = [];
var ys >> List(i64) = [1, 2, 3];
# var bad >> List(i64) = [1, "no"];  # type error per element
```

`List(T)` widens to `List(Any)`. `List(Any)` does not narrow to `List(T)`.

Index reads `List(T)[n]` as `T`. Index assignment and `@list_push` check the element type. `@clone` / `@move` keep `List(T)`.

```sprs
fn use_ints(xs >> List(i64)) {
  @list_push(xs, 3);
  xs[0] = 10;
}
```

## Comments and literals

Line comments run from `#` to the end of the line. `#define Windows` and `#define Linux` are preprocessor tokens (`priority = 4`) and win over ordinary comments. See [Modules](../reference/modules.md) for `#define`.

Integer literals match `[0-9]+` and are `i64`.

Floating-point literals match `[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?` and `[0-9]+[eE][+-]?[0-9]+` and are `f64`.

String literals are `"..."`. Escapes: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`, `\'`, and `\u{XXXX}` (1–6 hex digits). Unknown escapes and a dangling `\` are kept as written.

Booleans are `true` and `false`.

List literals use the same form as the `[1, 2, 3]` example under Variables and assignments.

## Identifiers and naming (`SPRS-SEM-025`)

Identifiers are `[A-Za-z_][A-Za-z0-9_]*`. A trailing `!` is not part of a name.

ASCII only. Digits may appear after the first character. Acronyms are ordinary words (`DmaController`, not `DMAController`). `__` prefix is rejected in every user category. `_` alone is the match wildcard only. `_name` is allowed only for local variables, parameters, and pattern binders.

| Category | Style | Examples |
|----------|-------|----------|
| function, module/`pkg`/`import`, global `var`, field, attach slot, macro | `snake_case` | `start_dma`, `fn_builds`, `@buf_len` |
| local variable / parameter / binder | `snake_case`, optional `_name` | `item`, `_tmp` |
| struct, closed label set, FunctionBuild, named type, type parameter | `PascalCase` | `Point`, `T`, `DmaController` |
| open label (`:name`) and closed member | `snake_case` | `:ready`, `:ConnectionState.waiting_for_dma` |

Static labels are split for checking: `:ready` is snake_case; `:ConnectionState.waiting_for_dma` is PascalCase set + snake_case member. Dynamic `:"..."` text is not checked. Canonical-name generation / automatic rename is not implemented.

Violations are `SPRS-SEM-025` with a category message such as `function names must use snake_case`.

## Keywords and `^`

Keywords are always keywords. Bare keyword in an identifier position is `SPRS-SYN-002` (`\`keyword\` is a reserved keyword`) with `help: use ^keyword if this name is intentional`.

Escape with `^` to use a keyword as a name. `^` is not part of the name. `new(expr)`, `destroy(expr)`, `exist(expr)`, and `defer expr;` stay core syntax; the same names as identifiers require `^new` and so on.

`@name` is a macro token. `@^name` is a lexer error.

Escaping a non-keyword that already passes its category rule is `SPRS-SYN-008` (`unnecessary identifier escape \`^foo\``) with `help: use foo instead of ^foo`. A bad name such as `^BadName` as a local reports `SPRS-SEM-025` first, not the unnecessary escape.

Keywords include: `if` `else` `while` `fn` `use` `function_build` `private` `return` `pkg` `import` `var` `pub` `struct` `ambi` `new` `destroy` `exist` `unsafe` `defer` `match` `case` `break` `true` `false` `bool` `str` `unit` `i8`…`u64` `f16` `f32` `f64` `label` `init` `source` `params` `return_type` `visibility` `type_param` `when` `is` `neq` `and` `or` `not`. `label` is for declarations (`label Color { ... }`, `label :ready;`); type position uses `Label`. `ambi` is the only remaining type-position keyword besides the primitive type names above.

A lone `^`, `^1`, or `^^name` is a lexer error.

## Variables and assignments

```sprs
# Comments start with a hash symbol
var x = 10;
var name = "sprs";
var is_valid = true;
var numbers = [1, 2, 3];
var ints >> List(i64) = [1, 2, 3];


# Not initialized variable
var y;  # y is initialized to Unit type

# Re-assignment

var y;
y = 20;
y = "now a string"; # y is now a string

```

Heap values move on assignment, call, and return. Use `@clone` to keep a copy. See [Memory Management](../reference/memory-management.md).

## Typed bindings and `ambi`

Parameter and return types use `>>` annotations. Unannotated parameters stay dynamic.
Annotated parameters are checked at call sites (arity and type).

```sprs
fn add(a >> i64, b >> i64) >> i64 {
  return a + b;
}
```

Fixed annotations reject incompatible reassignment. Prefix the type with `ambi`
(ambiguous) when the binding should start as that type but allow dynamic reassignment:

```sprs
fn demo(fixed >> i64, flex >> ambi i64) {
  fixed = 1;      # ok
  flex = 1;       # ok
  flex = "x";     # ok — becomes dynamic after reassignment
}
```

Applied types nest and are checked by constructor name and each argument:

```sprs
fn take(xs >> List(i64)) >> List(i64) {
  return xs;
}

fn take_job(job >> Process(str)) >> Process(str) {
  return job;
}

fn parse() >> Label(:error, Any) {
  return {:error, "no"};
}
```

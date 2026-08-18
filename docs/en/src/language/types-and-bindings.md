# Types and Bindings

## Basic data types

- Int (i64) — annotation keyword `int` (compatible with `i64` in type checks)
- Float (f64) — annotation keyword `fp` (compatible with `fp64` / `f64` in type checks)
- Bool — `bool`
- Str — `str`
- List (dynamic array) — annotation keyword `list` (also `List(T)` application form)
- Range — `range`
- Unit — `unit`
- Enum — compile-time frame; variants are Atoms (`Color.Red`)
- Struct
- Buffer — fixed-size zero-initialized byte array; annotation keyword `buffer`
- RawPtr — bare address from `@raw(buf)`; annotation keyword `rawptr`
- Error labels (catchable) — `err` sugar for `Label(:error, any)`
- Atom (immutable name) — annotation keyword `atom` (also `Atom(:name)` application form)
- Label (tagged value) — annotation keyword `label` (also `Label(:name[, T])` application form)
- i8 / u8 / i16 / u16 / i32 / u32 / i64 / u64 (mainly `@cast`; also usable in `>>` annotations)
- fp16 / fp32 / fp64 (mainly `@cast`; also usable in `>>` annotations)


## Comments and literals

Line comments run from `#` to the end of the line. `#define Windows` and `#define Linux` are preprocessor tokens (`priority = 4`) and win over ordinary comments. See [Modules](../reference/modules.md) for `#define`.

Integer literals match `[0-9]+` and are `i64`.

Floating-point literals match `[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?` and `[0-9]+[eE][+-]?[0-9]+` and are `f64`.

String literals are `"..."`. Escapes: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`, `\'`, and `\u{XXXX}` (1–6 hex digits). Unknown escapes and a dangling `\` are kept as written.

Booleans are `true` and `false`.

List literals use the same form as the `[1, 2, 3]` example under Variables and assignments.

Type *application* in annotations uses `Name(Type, …)` (for example `List(int)`, `Result(int, err)`, `Label(:ok, int)`). These are compile-time forms only: they are not runtime tags.

Everyday code keeps the flat keywords (`list`, `err`, `atom`, `label`, `buffer`, `rawptr`). Generics / type parameters (`Param`) are not user-facing yet.

## Keyword identifiers

Keywords stay lexer tokens, but many parse as names wherever an identifier is expected (`pkg`, `import`, `fn` name, parameters, `var`, fields, calls, variable references).

Usable as names without escaping (e.g. `pkg buffer;`, `fn buffer(new >> int)`, `var rawptr = new;`):
`fn`, `case`, `break`, `pkg`, `import`, `var`, `pub`, `enum`, `struct`, `cp`, `ambi`,
`unsafe`, `int`, `fp`, `bool`, `str`, `list`, `buffer`, `rawptr`, `range`, `unit`,
`err`, `label`, `atom`. Parameter names may also be `new` / `destroy` / `exist`.
`var defer = …` is allowed. A bare `new` / `destroy` / `exist` in an expression is a
variable; `new(4)` / `destroy(x)` / `exist(x)` stay the heap forms.

Still syntax when written bare: `if`, `else`, `while`, `match`, `return`, `true`,
`false`. `defer` is a `var` name only (not an expression identifier). `i8`…`u64` /
`fp16`…`fp64` stay type atoms for `@cast`. In `>>` position a type keyword is still
the type (`>> buffer` is Buffer, not a named type).

## `^` escape

Prefix any identifier with `^` to treat it as a normal name, including keywords
that cannot appear bare. The `^` is not part of the name: `^fn` and `fn` are the
same identifier. Examples: `var ^fn = 1;` / `^fn = ^fn + 1;`. A lone `^`, `^1`,
or `^^fn` is a lexer error.

## Variables and assignments

```sprs
# Comments start with a hash symbol
var x = 10;
var name = "sprs";
var is_valid = true;
var numbers = [1, 2, 3];


# Not initialized variable
var y;  # y is initialized to Unit type

# Re-assignment

var y;
y = 20;
y = "now a string"; # y is now a string

```

## Typed bindings and `ambi`

Parameter and return types use `>>` annotations. Unannotated parameters stay dynamic.
Annotated parameters are checked at call sites (arity and type). `int` and `i64` are
treated as the same type for checking; `fp` and `fp64` likewise.

```sprs
fn add(a >> int, b >> int) >> int {
  return a + b;
}
```

Fixed annotations reject incompatible reassignment. Prefix the type with `ambi`
(ambiguous) when the binding should start as that type but allow dynamic reassignment:

```sprs
fn demo(fixed >> int, flex >> ambi int) {
  fixed = 1;      # ok
  flex = 1;       # ok
  flex = "x";     # ok — becomes dynamic after reassignment
}
```

Applied types nest and are checked by constructor name and each argument:

```sprs
fn take(xs >> List(int)) >> List(int) {
  return xs;
}

fn parse() >> Result(int, err) {
  return 1;
}
```

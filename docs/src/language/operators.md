# Operators

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Increment/Decrement: `++`, `--` (only for postfix)
- Range creation: `..` (e.g., `1..10`)
- indexing: `list[index]` / `buf[index]` (Buffer uses byte get/set)
- Unary minus: `-x` (`Expr::Neg`).
- String concatenation: `str + str` calls `__string_concat`. It is not integer addition.
- There are no bitwise operator tokens. Shifts and logical not are macros: [`@lshift`](../reference/built-in-macros.md), [`@rshift`](../reference/built-in-macros.md), [`@not`](../reference/built-in-macros.md).

Integer `+`, `-` and `*` overflow, and `/` / `%` overflow checks, are described in [Errors](errors.md).
Division by zero keeps the existing `{:error, "Division by zero"}` label.
`++` and `--` are not covered by the integer-overflow contract.

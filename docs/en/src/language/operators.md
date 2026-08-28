# Operators

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Increment/Decrement: `++`, `--` (only for postfix)
- Range creation: `..` (e.g., `1..10`)
- indexing: `list[index]` / `buf[index]` (Buffer uses byte get/set). `List(T)[i] = v` type-checks `v` against `T`.
- Unary minus: `-x` (`Expr::Neg`).
- Prefix dereference: `*p` (`Expr::Deref`). Same unary precedence as `-x`. `*p` reads the pointee; `*p = value` is an assignment target (`Stmt::DerefAssign`). Nested `**pp` is allowed.
- Pointer addition: `Ptr(T) + offset` is element-sized. The stride is `size_of(StorageRep(T))` from the LLVM target ABI (including struct padding). `Ptr(MaybeUninit(T))` uses the same stride as `T`. `offset` is `usize` or a non-negative integer literal. Overflow panics with `Pointer arithmetic overflow`. `integer + Ptr(T)`, negative literals, and `Ptr(T)` with `-` `*` `/` `%` are `SPRS-TYP-001`.
- String concatenation: `str + str` calls `__string_concat`. It is not integer addition.
- There are no bitwise operator tokens. Shifts and logical not are macros: [`@lshift`](../reference/built-in-macros.md), [`@rshift`](../reference/built-in-macros.md), [`@not`](../reference/built-in-macros.md).

Integer `+`, `-` and `*` overflow, and `/` / `%` overflow checks, are described in [Errors](errors.md).
Division by zero keeps the existing `{:error, "Division by zero"}` label.
`++` and `--` are not covered by the integer-overflow contract.

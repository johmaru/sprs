# Memory Management

Sprs uses **move semantics** for heap values (`str`, `list`, `range`, `struct`, `label`, `buffer`).
Assigning or passing one of these values transfers ownership; the old binding becomes invalid
(`Unit`). Integers, floats, and bools are copied instead.

Use `@clone(x)` when you need to keep the original value after a move.
Use `cp var` when the same binding is read many times and writing `@clone` each time is noisy.
Use `@move(x)` to opt out of that sugar for one use.

Auto-clone from `cp` applies when ownership would otherwise move: function arguments,
`@println` / `@list_push`, assignment RHS, `var` / `cp var` init from another variable,
and `return`. It does **not** rewrite every expression operand (for example `a + b`).

**Phase 1:** `cp` is intended mainly for `str`. Other heap types still work, but each use
deep-copies; the compiler warns when `cp` is clearly applied to `list` / `range` / `struct`.

**Move on assignment:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    var copy = greeting;       # ownership moves to copy; greeting is now invalid
    @println(copy);            # prints: Hello, Sprs!
}
```

**Move into a function call:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    @println(greeting);        # greeting is moved into @println and becomes invalid
}
```

**Keep ownership with `@clone`:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    @println(@clone(greeting)); # prints a copy; greeting stays valid
    @println(greeting);         # still prints: Hello, Sprs!
}
```

**Always-clone binding with `cp var`:**

```sprs
fn main() {
    cp var greeting = "Hello, Sprs!";
    @println(greeting);         # same as @println(@clone(greeting))
    @println(greeting);         # still valid
    @println(@move(greeting));  # one-shot real move; greeting becomes Unit
}
```

Buffers participate in the same auto-drop path as other heap values. Prefer `destroy` / `defer destroy(...)` when you need an explicit lifetime cut. Details of Buffer liveness, `unsafe`, RawPtr, and `defer` order are in [Buffers and Unsafe](../language/buffers-and-unsafe.md).

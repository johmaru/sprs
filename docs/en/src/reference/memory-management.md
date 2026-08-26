# Memory Management

Sprs uses **move semantics** for heap values (`str`, `List`, `Range`, struct, label, `Buffer`).
Assigning or passing one of these values transfers ownership; the old binding becomes invalid
(`Unit`). Integers, floats, and bools are copied instead.

Use `@clone(x)` to keep the original after a use that would move. Use `@move(x)` to move out of
a variable explicitly (the binding becomes `Unit`). There is no `cp var`. Ordinary `var`
bindings always move.

**List index is a move:**

`values[index]` takes ownership of the element and leaves `Unit` in that list slot.
Reading the same index again yields `Unit`. To keep the original list's element,
`@clone` the list first and read from the clone.

```sprs
fn main() {
    var values = [];
    @list_push(values, "hello");
    var first = values[0];     # moves the string out; values[0] is now Unit
    @println(first);           # prints: hello
    @println(values[0]);       # prints: ()
}
```

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

**Explicit `@move`:**

```sprs
fn main() {
    var greeting = "Hello, Sprs!";
    @println(@move(greeting));  # greeting becomes Unit
}
```

**Typed pointer dereference (`Ptr(T)`):**

A non-owning read of `*p` keeps the pointee in place. Contexts that take ownership — variable initialization (`var x = *p`), arguments, `return`, and storing into a list, label, or struct — clone the pointee with the existing `__clone` path.

Replacement assignment `*p = value` prepares the owned right-hand side first, drops the old pointee, then stores the new value. `*p = *p` therefore clones before drop. The pointer value itself is a non-owning address and is not an extra drop target.

`@move(*p)` is not implemented (`@move` still requires a variable). Initialization of uninitialized pointee storage is not implemented.

Buffers participate in the same auto-drop path as other heap values. Prefer `destroy` / `defer destroy(...)` when you need an explicit lifetime cut. Details of Buffer liveness, `unsafe`, RawPtr, and `defer` order are in [Buffers and Unsafe](../language/buffers-and-unsafe.md).

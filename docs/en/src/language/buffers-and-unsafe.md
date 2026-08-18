# Buffers and Unsafe

## Buffers

`new(n)` allocates a zero-initialized Buffer of `n` bytes (negative → invalid handle; `0` is a valid empty buffer).
Bytes are Integers in `0..=255`. Index sugar `buf[i]` reads/writes like `@buf_get` / `@buf_set`.
Writes truncate to the low 8 bits. Out-of-bounds `@buf_get` / `buf[i]` reads return the `Unit` sentinel
(same convention as list indexing); out-of-bounds writes are no-ops.

`destroy(x)` explicitly releases a heap value and marks the binding `Unit` (double `destroy` is a no-op).
`exist(x)` is `true` only while `x` is a live Buffer. Scope exit still auto-`__drop`s live Buffers, so
explicit `destroy` is optional.

```sprs
var a = new(4);
@buf_set(a, 0, 10);
a[1] = 20;
@println(@buf_len(a));           # 4
@println(a[0] + @buf_get(a, 1)); # 30
@println(exist(a));             # true
destroy(a);
@println(exist(a));             # false
```

## Buffers, destroy, and exist

Buffers participate in the same auto-drop path as other heap values: leaving a scope without
`destroy` still frees a live Buffer. Prefer `destroy` / `defer destroy(...)` when you need an
explicit lifetime cut; `exist` reports Buffer liveness only.

See [Memory Management](../reference/memory-management.md) for move semantics and automatic drop.

## Unsafe, RawPtr, and defer

`@raw` / `@free` are allowed only inside `unsafe { ... }` (nesting increments a depth counter).
`@raw(buf)` moves the Buffer's byte allocation to a RawPtr (bare address). After `@raw`, the
source binding is `Unit`, so later auto-drop / `destroy` on that binding is a no-op.
The caller owns the address and must `@free` it. Empty / non-Buffer / stale inputs yield a null
RawPtr (`0`); `@free` ignores null and unknown addresses.

`defer <expr>;` queues `expr` and runs the queue **LIFO** at scope exit, **before** automatic
variable drops (including on `return`).

```sprs
fn demo() {
  var a = new(1);
  defer destroy(a);   # runs at scope exit before auto-drop
  @buf_set(a, 0, 1);

  var b = new(2);
  defer destroy(b);
  unsafe {
    var p = @raw(b);  # b becomes Unit; deferred destroy(b) is then a no-op
    @free(p);
  }
}
```

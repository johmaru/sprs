# Built-in Macros

Macro names have the form `@[A-Za-z_][A-Za-z0-9_]*`. A lone `@` is a lexer error. An unknown name is `SPRS-SEM-003` (`Unknown macro: ...`).

* `@println(value)`: Print value to the console

examples:

```sprs
@println(y[1]);
```

* `@list_push(list, value)`: Push value to the end of the list

examples:

```sprs
@list_push(y, z);
```

* `@buf_len(buf)`: Buffer length as Integer (`0` for stale / non-Buffer)
* `@buf_get(buf, i)`: read one byte as Integer; OOB / stale → `Unit`
* `@buf_set(buf, i, v)`: write low 8 bits of `v` at `i`; OOB → no-op

examples:

```sprs
var a = new(2);
@buf_set(a, 0, 7);
@println(@buf_get(a, 0));
@println(@buf_len(a));
```

See [Buffers and Unsafe](../language/buffers-and-unsafe.md) for Buffer allocation, indexing, `destroy`, `exist`, `unsafe`, RawPtr, and `defer`.

* `@raw(buf)`: move Buffer ownership to a RawPtr. Requires `unsafe { ... }`.
  Source binding becomes `Unit`; caller must `@free` the result.
* `@free(p)`: release a RawPtr from `@raw`. Requires `unsafe { ... }`.
  Null / unknown addresses are no-ops; source binding becomes `Unit`.

examples:

```sprs
var b = new(2);
unsafe {
  var p = @raw(b);
  @free(p);
}
@println(exist(b)); # false
```

* `@clone(value)`: Clone the value

examples:

```sprs
var a = "hello";
@println(@clone(a));

```

* `@move(value)`: Move out of a variable (invalidates the binding)

examples:

```sprs
var a = "hello";
@println(@move(a)); # a becomes Unit
```

See [Memory Management](memory-management.md) for move semantics, `@clone`, `@move`, and automatic drop.

* `@cast(value, type)`: Cast the value to the specified type

examples:

```sprs
var a = 100; # default is i64
var b = @cast(a, i8); # cast to i8
@println(b); # prints 100 as i8
```

* `@fcast(value)`: Explicitly convert an integer, `bool`, or `str` value to `str`.
  Unsupported values return the catchable error `TypeError: unexpected tag in @fcast`;
  an existing error is returned unchanged. No implicit string conversion is performed.

```sprs
var ok = 5 == 5;
@println("bool test : " + @fcast(ok)); # bool test : true
```

* `@lshift(value, shift_amount)`: exactly two arguments. Integer tags only. Signed tags (`Integer`, `i8`, `i16`, `i32`, `i64`) use `shl` / arithmetic right shift. Unsigned tags (`u8`..`u64`) use `shl` / logical right shift. A non-integer value produces the error label `"@lshift expects an integer value"`. An existing error-label argument is returned unchanged. The result keeps the tag of `value`.
* `@rshift(value, shift_amount)`: same rules as `@lshift`. The non-integer message is `"@rshift expects an integer value"`.
* `@not(value)`: exactly one argument. Boolean `true` when `data == 0`, otherwise `false`. This is not bitwise complement.
Struct initialization is the core form `init TypeName { field = value, ... }`, not a macro. `@init` is an unknown macro (`SPRS-SEM-003`). See [Structs](../language/structs.md).


* `@attach(expr, <:name)`: Clone `expr` into the function-local attach slot `<:name`.
  Read the captured value with `<:name` (not bare `:name`). Dynamic slot names are not supported.

```sprs
@attach(compute(), <:result);
@println(<:result);
```

* `@label_is(value, expected)`: `true` when `value` is a label whose name matches
  `expected` (an Atom: `:name` or `:"{ident}-…"`).
* `@label_payload(value)`: Clone the label payload (Unit when not a label).
* `@label_name(value)`: Return the label name as `str` (`""` when not a label).

```sprs
var v = {:ok, 1};
if @label_is(v, :ok) {
  @println(@label_payload(v));
  @println(@label_name(v));
}
```

See [Labels and Match](../language/labels-and-match.md) for Atom/Label syntax, attach slots, and `match`.

* `@error(reason)`: create `{:error, reason}` with exactly one argument.
* `@is_error(value)`: `true` when `value` is an error label.
* `@error_message(value)`: String payload directly when the reason is a String; other payloads are rendered using the normal value formatter.

See [Errors](../language/errors.md) for `Label(:error, T)`, `?`, uncaught `main` errors, integer overflow, and division by zero.

**Note:** `@cast` macro is faster than normal int type, because it use i8 and u8 llvm type directly.

examples:

```sprs
var i = 0; # default is i64
while i < 5 {
  @println(i); ## this is too slow for embedded and system programming environment, because it use dynamic type checking.
 i = i + 1;
}
```

but with `@cast` macro

```sprs
var i = @cast(0, i8); # i is i8 type
while i < @cast(5, i8) {
 @println(i); ## this is faster for embedded system, because it use i8 llvm type directly.
i = i + @cast(1, i8);
}
```

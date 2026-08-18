# Functions and Control Flow

## Functions

```sprs
fn add(a, b) {
   return a + b;
}

fn main() {
 result = add(5, 10);
 @println(result);
}
```

If a function is not marked as `pub`, it is a private function.
The function can be called in the same module.

Parameter and return types use `>>` annotations. Unannotated parameters stay dynamic.
Annotated parameters are checked at call sites (arity and type). `int` and `i64` are
treated as the same type for checking; `fp` and `fp64` likewise.

```sprs
fn add(a >> int, b >> int) >> int {
  return a + b;
}
```

See [Types and Bindings](types-and-bindings.md) for `ambi` and applied types in signatures.

## Control flow

```sprs
if x > 5 {
  @println("x is greater than 5");
} else {
 @println("x is 5 or less");
}

while x < 10 {
 println(x);
 i++;
}
```

`else` is optional. There is no `elseif` keyword. Chain conditions with nested `if` or consecutive `if` statements, as in `if_elseif_chain` in `tests/src/control_flow.sprs`:

```sprs
fn if_elseif_chain() >> i64 {
    var x = 15;
    if x < 10 {
        return 1;
    }
    if x < 20 {
        return 2;
    }
    if x < 30 {
        return 3;
    }
    return 0;
}
```

`return` exits the current function.

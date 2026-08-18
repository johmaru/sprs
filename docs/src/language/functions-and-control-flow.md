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

## FunctionBuild

Function contracts can be declared separately from the function body with
`function_build`. The function then attaches one build with `use`. After
semantic analysis this is equivalent to writing the same params, return type,
and visibility on a normal `fn` header. Existing inline `fn` syntax is unchanged.

```sprs
function_build AddBuild {
    @FbArgs(lhs >> i64, rhs >> i64);
    @FbRetTy(i64);
    @FbVisibility(pub);
}

fn add use AddBuild {
    return lhs + rhs;
}
```

Phase 1 directives:

- `@FbArgs(...)` names parameters and their `>>` type annotations.
- `@FbRetTy(T)` sets the return type.
- `@FbVisibility(pub)` or `@FbVisibility(private)` sets the visibility of the
  generated function.

These `@Fb*` forms are compile-time directives, not runtime macros. Omitting them
means zero arguments, no return annotation, and private.

A function may use exactly one build. `fn name use Build { ... }` cannot mix
inline parameters, a `>>` return annotation, or `pub fn` on the same header.
FunctionBuild names live in their own namespace, so `function_build Foo` and
`fn Foo use Foo` may coexist. The same build may be reused by several functions
and may be declared after the functions that use it.

`pub function_build` is the visibility of the **build declaration** (whether
another source can attach it). It is independent of `@FbVisibility`, which is
the visibility of functions that use the build.

To attach builds declared in another file, name that file as a FunctionBuild
source. The compiler reads `{source_path}/contracts.sprs`; it does not perform
a runtime `import`.

```sprs
# contracts.sprs
pkg contracts;

struct Job {
    id >> i64
}

pub function_build AddBuild {
    @FbArgs(lhs >> i64, rhs >> i64);
    @FbRetTy(i64);
    @FbVisibility(pub);
}

function_build InternalBuild {
    @FbRetTy(str);
}

fn helper_not_imported() {
}
```

```sprs
# consumer.sprs
pkg consumer;

#define FunctionBuild contracts

fn add use AddBuild {
    return lhs + rhs;
}
```

Source rules:

- Only `pub function_build` declarations are visible to the consumer, referenced
  by unqualified name (`AddBuild`, not `contracts.AddBuild`).
- A file may contain at most one `#define FunctionBuild`.
- Nested FunctionBuild sources are allowed; cycles are not.
- Ordinary functions in the source are not imported or compiled into the
  consumer.
- Structs declared in the source may be used as named types in `@FbArgs` and
  `@FbRetTy`.

Phase 2 forms (`fbtype`, unification, `fbif`) are not available.

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

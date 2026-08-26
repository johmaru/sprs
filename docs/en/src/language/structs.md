# Structs

A bare struct name can be used in type annotations for fields, function
arguments, and return values. `Self` resolves to the struct currently being
declared. It is valid in that struct's field types, including nested type
applications such as `List(Self)`, and in method parameter / return
annotations and bodies. Struct types in the same module can be referenced
regardless of declaration order. An undefined bare type name, or `Self`
outside a struct field or method, is reported as `SPRS-SEM-011`.

```sprs
struct Tree {
  value >> i64,
  children >> List(Self)
}

fn identity(value >> Tree) >> Tree {
  return value;
}
```

## `init`

Struct values are created with `init TypeName { field = expr, ... }`. Trailing
commas are allowed. `init Empty {}` is valid for a zero-field struct.

Fields are filled in declaration order. An explicit initializer wins. An omitted
field uses `StructField.default_value`, evaluated at each `init` in the caller
expression context. There is no `self` / earlier-field binding in a default
expression. A field with no default and no initializer is
`missing required field \`name\` in init Type`. Unknown and duplicate fields are
compile errors on the initializer span. Old `@init(...)` is not struct
initialization; `@init` is an unknown macro.

```sprs
pub struct Point {
  x >> i64,
  y >> i64
}

struct Counter {
  value >> i64 = 0,
  name >> str
}

struct Empty {}

fn main() {
  var p = init Point {
    x = 10,
    y = 20,
  };
  @println(p.x);
  @println(p.y);

  var c = init Counter {
    name = "counter",
  };
  @println(c.value); # 0 from the default

  var e = init Empty {};
}
```


## Generic structs

A struct may declare one or more PascalCase type parameters after its name:
`struct Pair(T)` or `struct Pair(A, B)`. Field types may mention those
parameters. A generic struct is used only through an explicit concrete
application such as `Pair(i64)` or `init Pair(str) { ... }`. The compiler
substitutes the arguments at compile time and produces a distinct concrete
layout for each distinct argument list (`Pair(i64)` is not `Pair(f64)`).
Nested applications such as `Pair(Pair(i64))` specialize the inner type first.

Owned string specializations use `str` (`init Pair(str) { a = "owned", b = "x" }`).
`String` is not a type name. Generic field defaults and `init Pair { ... }` without
type arguments (including inference from an expected type) cannot be used.

```sprs
struct Pair(T) {
  a >> T,
  b >> T
}

fn use_pair() {
  var nums = init Pair(i64) { a = 1, b = 2 };
  var owned = init Pair(str) { a = "owned", b = "x" };
  var nested = init Pair(Pair(i64)) {
    a = init Pair(i64) { a = 1, b = 2 },
    b = init Pair(i64) { a = 3, b = 4 },
  };
}
```

## Methods

Methods are declared inside the struct, after the fields. If the struct has
any method, the last field must have a trailing comma. The first parameter
must be unannotated `self`. The receiver is moved like a normal argument;
there is no implicit borrow or clone.

```sprs
struct Pair(T) {
  a >> T,
  b >> T,
  pub fn get(self) >> T {
    return self.a;
  }
}

fn main() {
  var pair = init Pair(i64) { a = 1, b = 2 };
  pair.get();
  make_pair().get();
}
```

`Self` and the owner's type parameters are substituted with the concrete owner
(`Pair(i64)` for `init Pair(i64) { ... }`). Nested methods cannot use
FunctionBuild, static methods, overloads, or method-specific type parameters.
A method call on a non-struct receiver is `SPRS-TYP-007`
(`method call requires a struct receiver, found X`).

Only a public method on a public struct is visible to importers. A private
method can be called only in the declaring module. Calling a private method
from an importer is `SPRS-SEM-015` (`Undefined function: {name}`).

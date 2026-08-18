# Enums and Structs

## Enum frames

```sprs
pub enum Animal {
 Dog,
 Cat,
}

fn main() {
   @println(Animal.Dog);

}

```

A source `enum` is a compile-time frame only. `Color.Red` is a runtime Atom whose
intern key is `"Color.Red"` (immediate intern id, not a slab handle). Runtime tag
`7` (former `Tag::Enum`) is unused.

- `Color.Red == Color.Red` is true; `Color.Red == :Red` is false (keys `"Color.Red"` vs `"Red"`).
- `match` on an enum-typed scrutinee uses `case :Red` (bare variant). The compiler
  compares against the framed key and requires every variant or a trailing `case _`
  (error `non-exhaustive match on Color; missing Green, Blue`). Open Atom / Label
  matches stay runtime-checked (`Match failed`).
- Duplicate `enum` names in one compilation are a semantic error. Non-`pub`
  variants are not visible from other modules.

```sprs
pub enum Color {
  Red,
  Green,
  Blue,
}

fn enum_match_red() >> int {
  var r = match Color.Red {
    case :Red => 1
    case :Green => 2
    case :Blue => 3
  };
  return r;
}
```

## Grouped label enum-compatible declarations

Grouped `label` declarations provide enum-compatible syntax for namespaced Atoms.
`pub label :Color{:red, :blue}` creates the same kind of compile-time frame as a
source `enum`, exports it, and exposes its variants as `Color.red` and `Color.blue`.
Both declaration forms produce framed Atom intern keys at runtime.

```sprs
pub label :Color{:red, :blue}

fn print_grouped_label_color() {
  @println(Color.red);
}
```

## Structs

A bare struct name can be used in type annotations for fields, function
arguments, and return values. `Self` resolves to the struct currently being
declared and is valid only inside that struct's field types, including
nested type applications such as `List(Self)`. Struct types in the same
module can be referenced regardless of declaration order. An undefined bare
type name, or `Self` outside a struct field, is reported as `SPRS-SEM-011`.

```sprs
struct Tree {
  value >> i64,
  children >> List(Self)
}

fn identity(value >> Tree) >> Tree {
  return value;
}
```

```sprs
pub struct Point {
  x >> i64,
  y >> i64
}

fn main() {
 var p = @init(Point {
  x = 10,
  y = 20
 });

@println(p.x); # prints 10
@println(p.y); # prints 20
}
```

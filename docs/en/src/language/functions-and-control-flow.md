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
Annotated parameters are checked at call sites (arity and type).

```sprs
fn add(a >> i64, b >> i64) >> i64 {
  return a + b;
}
```

See [Types and Bindings](types-and-bindings.md) for `ambi` and applied types in signatures.
Function, parameter, and local names use `snake_case` (`SPRS-SEM-025`).

## FunctionBuild

Function contracts are declared with `function_build` and attached with `fn name use Build`.
After analysis this is the same params, return type, and visibility as an inline `fn` header.
Inline `fn` syntax is unchanged. Old `@Fb*` directives and `#define FunctionBuild` are gone.

```sprs
function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
    visibility(pub);
}

fn add use AddBuild {
    return lhs + rhs;
}
```

Directives (each of `params` / `return_type` / `visibility` at most once):

- `params(...)` — parameter names and `>>` types. Omitted means zero arguments.
- `return_type(T)` — return type. Omitted means unannotated / `Any`.
- `visibility(pub)` or `visibility(private)` — visibility of functions that use the build. Omitted means private.
- `type_param T;` — PascalCase type parameter. Duplicate names, clashes with builtins / visible structs / closed label sets, and uses of undeclared params are compile errors. In the build body, declared PascalCase params become `Type::Param`.
- `when CONDITION { return_type(T); }` — one `return_type` only. No `params` / `visibility` / nested `when` / macros inside the block.

A function may use exactly one build. `fn name use Build { ... }` cannot mix
inline parameters, a `>>` return annotation, or `pub fn` on the same header.
FunctionBuild names live in their own namespace, so `function_build Foo` and
`fn foo use Foo` may coexist (the `fn` itself is snake_case). The same build may
be reused by several functions and may be declared after the functions that use it.

`pub function_build` is the visibility of the **build declaration** (whether
another source can attach it). It is independent of `visibility(...)`, which is
the visibility of functions that use the build.

To attach builds declared in another file, name that file as a FunctionBuild
source. The compiler reads `{source_path}/name.sprs`; it is not a runtime `import`.
`function_build` is a keyword, so a package named `function_build` must be escaped
or renamed (`fn_builds`).

```sprs
# fb_contracts.sprs
pkg fb_contracts;

struct Job {
    id >> i64
}

pub function_build AddBuild {
    params(lhs >> i64, rhs >> i64);
    return_type(i64);
    visibility(pub);
}

function_build InternalBuild {
    return_type(str);
}

fn helper_not_imported() {
}
```

```sprs
# consumer.sprs
pkg consumer;

function_build source fb_contracts;

fn add use AddBuild {
    return lhs + rhs;
}
```

Source rules:

- Only `pub function_build` declarations are visible to the consumer, referenced
  by unqualified name (`AddBuild`, not `fb_contracts.AddBuild`).
- A file may contain at most one `function_build source` (`SPRS-SEM-023`).
- Nested FunctionBuild sources are allowed; cycles are not (`SPRS-SEM-024`).
- Ordinary functions in the source are not imported or compiled into the
  consumer.
- Structs declared in the source may be used as named types in `params` and
  `return_type`.

### Call-contract solver

Call sites (`check_call_arguments` and return-type inference) share one resolver:

1. Arity (`SPRS-SEM-016` on mismatch).
2. Unify each parameter pattern with the actual type. `Type::Param` binds;
   `Any` is weak and a later concrete type overwrites it; a conflicting concrete
   type is `SPRS-TYP-007`.
3. Every used type parameter must be concrete (not left as `Any`).
4. Evaluate `when` conditions.
5. Substitute into the chosen return type.

Conditions: `is` is canonical type compatibility after substitution; `neq` is
negation only when both sides are concrete; `and` / `or` / `not` short-circuit;
operands are types (including type parameters). Zero matching `when` rules fall
back to the unconditional `return_type`, or unannotated/`Any` if that is absent.
Two or more matches are `SPRS-TYP-007` (`multiple \`when\` rules matched`) even
when the result types are equal. A body that only has conditional returns is
checked as `Any`; the call-site type is the resolver result. There is no
monomorphization and no extra runtime tag.

```sprs
function_build Identity {
    type_param T;
    params(value >> T);
    return_type(T);
    visibility(pub);
}

fn identity_i64 use Identity {
    return value;
}

fn identity_str use Identity {
    return value;
}
```

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

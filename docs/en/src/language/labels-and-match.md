# Labels and Match

## Labels (tagged values)

Labels are a core feature for tagging values, not an error-only type.
Surface type `Label` covers both payloadless atoms and payload labels; it is not a single runtime tag.
Payloadless `:name` is runtime `Tag::Atom` (9). `{:name, payload}` is runtime `Tag::Label` (10).
Type-position `label` / `atom` are rejected (`SPRS-SEM-011`). Write `Label`, `:name`, or `Label(:name, T)`.
Closed members are snake_case (`:Color.red`, not `:Color.Red`).
A label always has a name plus one payload: `{:name, payload}`.
A bare `:name` is an immutable Atom (`Tag::Atom`) with no payload.
Declare module-global Atom constants with `label :ready;` and closed label
sets with `label Color { red, blue }`. Closed-set members are always written
fully qualified as `:Color.red`. Open Atoms stay `:ready` (or the exported
constant `ready`). Use `pub label ...` to export a declaration; otherwise it
stays module-local. A local variable of the same name shadows a standalone
Atom constant. `:Color.red` uses intern key `"Color.red"` and
`:Color.red == :red` is false. Unknown or private `:Foo.bar` is `SPRS-SEM-004`.
`label Color { red, blue }` is the only closed label set form. Empty sets are
a syntax error. The old `enum Color { Red }` and `label :Color{:red}` forms
are rejected. `enum` is a normal identifier.

```sprs
pub label :ready;                     # exported Atom constant
label Color { red, blue }             # closed label set
label :local_atom;                    # module-local Atom constant

var success_label = :ok;              # Atom
var labeled_value = {:ok, 42};        # Label with payload
var color = :Color.red;               # intern key "Color.red"
@println(ready == :ready);            # true
@println(color == :Color.red);        # true
@println(color == :red);              # false

var item_index = 10;
var dynamic_label = {:"{item_index}-item", 42};   # name becomes "10-item"

if @label_is(dynamic_label, :"{item_index}-item") {
  @println(@label_payload(dynamic_label));  # 42
  @println(@label_name(dynamic_label));     # "10-item"
}

fn wrap(value_input >> i64) >> Label {
  var item_index = value_input;
  return {:"{item_index}", value_input};
}
fn wrap_named(value_input >> i64) >> Label(:ok, i64) {
  return {:ok, value_input};
}
fn take(label_value >> Label) >> Label {
  return label_value;
}

@attach(wrap_named(7), <:item);   # capture into a local slot
@println(<:item);                 # {:ok, 7}
```

Notes:

- Dynamic templates reject `{}`, `{expr}`, and nested braces. Use `{ident}` only.
- `@attach(expr, <:name)` stores a cloned value into the function-local slot
  `<:name`; reading `<:name` before any `@attach` is a compile error.
- A bare `:name` is always an Atom and never shadows an attached slot.
- `?` propagates only the label named `:error`; ordinary labels such as `:ok` continue on the normal path.

## Match

`match` branches on Atom / Label values with static patterns. It comes in
two forms: a **statement** (blocks or a bind variable) and an
**expression** (produces a value).

Statement forms:

- **Bind** — `match <Expr> ?(var name) { case PAT => expr break; … }`.
  Each arm evaluates an expression, stores it into `name`, and leaves the
  match. The binding is visible after the match in the same block.
- **No bind** — `match <Expr> { case PAT => { stmts } … }`. Arms are
  statement blocks (same shape as `if`).

Expression form — `match <Expr> { case PAT => expr … }` produces the
matching arm's value (no `break`). It needs a context that consumes the
value (e.g. `var r = …;`); use a no-bind statement for a standalone branch.

Patterns (v1, static names only):

- `case :name` — match an open Atom or Label by name (no payload bind)
- `case :Set.member` — match a closed label set member (fully qualified)
- `case {:name, binder}` — Label only; bind the payload to `binder`
  (`_` discards it)
- `case _` — matches anything; must be the last arm. Use it as a default
  to avoid the `Match failed` panic.

```sprs
fn match_label_bind() >> i64 {
  match {:ok, 7} ?(var r) {
    case :ok => 1 break;
    case :error => 0 break;
  }
  return r;
}

fn match_payload_bind() >> i64 {
  match {:ok, 7} ?(var r) {
    case {:ok, x} => x break;
    case :error => 0 break;
  }
  return r;
}

fn match_atom_bind() >> i64 {
  match :ok ?(var r) {
    case :ok => 1 break;
    case :error => 0 break;
  }
  return r;
}

fn match_no_bind_block() >> i64 {
  var flag = 0;
  match :error {
    case :ok => { flag = 100; }
    case :error => { flag = 1; }
  }
  return flag;
}

fn match_expr_example(v >> Label) >> i64 {
  var r = match v {
    case :ok => 1
    case :error => 0
    case _ => -1
  };
  return r;
}

label State { idle, running }

fn match_closed_label_set() >> i64 {
  var r = match :State.idle {
    case :State.idle => 1
    case :State.running => 2
  };
  return r;
}
```

Notes:

- Unmatched scrutinees panic with `Match failed` (process exits non-zero)
  unless a trailing `case _` catches everything.
- A closed label set scrutinee is checked at compile time: every member as
  `:Set.member`, or a trailing `case _`. Missing members are listed fully
  qualified in declaration order (`non-exhaustive match on State; missing State.running`).
  A short `case :running` does not cover `State.running`.
- Dynamic name patterns such as `case :"{i}-item"` are rejected at compile
  time. Prefer `@label_is` with `if` for dynamic names.
- The bind marker is the single token `?(`; it does not collide with
  postfix Try `?` (`match x? { … }` still means Try-then-match).

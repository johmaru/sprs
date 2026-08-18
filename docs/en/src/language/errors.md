# Errors

Compile-time codes (`SPRS-SYN-001`, `--error-format`) are documented in [Compiler Errors](../reference/compiler-errors.md). This chapter describes runtime error labels.

## Error labels

Errors are ordinary labels, not a dedicated runtime value. Write `Label(:error, Any)` (or broad `Label`) in signatures. `@error(reason)` creates `{:error, reason}` with exactly one argument. There is no `err` type alias.

The same value can be created directly as a normal label literal. Use
`Label(:error, T)` when the error name and payload type should be part of the
function signature:

```sprs
fn make_error_label() >> Label(:error, str) {
  return {:error, "file not found"};
}

fn main() {
  var error_label_value = make_error_label();
  @println(@is_error(error_label_value));         # true
  @println(@error_message(error_label_value));    # file not found
}
```

`@error(reason)` is shorthand for the same label convention:

```sprs
fn make_error() >> Label(:error, Any) {
  return @error("file not found");
}

fn show_error() {
  var error_value = make_error();
  @println(@is_error(error_value));         # true
  @println(@error_message(error_value));    # file not found
  @println(@error_message(@error(:enoent))); # :enoent
}
```

`@error_message` returns the String payload directly when the reason is a
String; other payloads are rendered using the normal value formatter. The
removed `@error_code` macro and the legacy `Tag::Error`/`SlotData::Error` ABI
are no longer available. Runtime tag `9` is intentionally unused, while
`Tag::Label` remains `10`.

When an error label reaches the `main` boundary without being handled, Sprs
prints `Uncaught error in main` and exits. A known runtime limitation is that
the subsequent thread-local slot cleanup may emit a TLS destruction warning:
the cleanup of a label payload re-enters the same thread-local slot table after
it has started being destroyed. This warning occurs during process termination,
after the uncaught-error message, and does not change the error-label result.

`?` propagates only the label named `:error`; ordinary labels such as `:ok` continue on the normal path.

## Integer overflow

Integer `+`, `-` and `*` are checked against the sign and bit width of the
integer type, and `/` and `%` additionally check the signed-minimum / `-1`
combination before the operation runs. On success the result is the usual
integer value; on overflow the full label `{:error, :overflow}` is returned.
`@is_error`, `@label_payload`, `@error_message` and `?` all work on it in
the same way as on any other error label.

```sprs
fn propagate_overflow() >> i64 {
  var value = (9223372036854775807 + 1)?;
  return value;
}

fn inspect_overflow() {
  var value = 9223372036854775807 + 1;
  @println(@is_error(value));       # true
  @println(@label_payload(value));  # :overflow
  @println(@error_message(value));  # :overflow
}
```

Different integer tags still promote to the default `i64` as before; `++`
and `--` are not covered by this contract. Division by zero keeps the
existing `{:error, "Division by zero"}` label.

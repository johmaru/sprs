# catchable なエラー機構 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 実行時エラーを `Tag::Error` の slab 値として表現し、`?` 演算子で伝播、マクロで catch できるようにする。

**Architecture:** `Tag::Error = 9` を slab タグとして追加し、`data` に `SlotData::Error { code, message }` の slab ハンドルを格納する。現状6箇所の `create_panic_err` + `build_unreachable` を `create_error_value` に置換する。全タグディスパッチサイトに short-circuit ルールを追加する。`?` 後置演算子と4つのエラーマクロ（`@is_error`, `@error_code`, `@error_message`, `@error`）を導入する。

**Tech Stack:** Rust, inkwell (LLVM bindings), LALRPOP, logos (lexer), slab-based runtime

## Global Constraints

- コードコメントは英語で書く
- 変数名は3文字以上で説明的にする（`cat` → `error_cat`, `s` → `format_str` 等）
- `Tag` enum は `src/runtime/runtime.rs:38` と `src/llvm/compiler.rs:237` の2箇所に存在する。両方に `Error = 9` を追加する
- `SlotData` enum は `src/runtime/runtime.rs:97` にある
- `get_runtime_fn` は `src/llvm/compiler.rs:460` で、新関数を `match name` に追加する
- `create_panic_err` は `src/llvm/value.rs:20` にある
- ビルド確認は `cargo build` で行う
- テスト確認は `cargo test` で行う
- 既存テストスイート（87 PASS / 7 XFAIL / 8 FAIL）が回帰しないこと

---

## Task 1: Tag::Error と SlotData::Error の追加

**Files:**
- Modify: `src/runtime/runtime.rs:38-63` (Tag enum)
- Modify: `src/runtime/runtime.rs:97-113` (SlotData enum)
- Modify: `src/runtime/runtime.rs:66-75` (is_heap_tag)
- Modify: `src/llvm/compiler.rs:237-262` (Tag enum)

**Interfaces:**
- Produces: `Tag::Error = 9` in both `runtime.rs` and `compiler.rs` Tag enums. `SlotData::Error { code: u32, message: Option<String> }` in runtime.rs. `is_heap_tag` returns `true` for `Tag::Error`.

- [ ] **Step 1: Add `Error = 9` to Tag enum in runtime.rs**

```rust
// src/runtime/runtime.rs, in the Tag enum after Struct = 8
    Error = 9,
```

- [ ] **Step 2: Add `Error = 9` to Tag enum in compiler.rs**

```rust
// src/llvm/compiler.rs, in the Tag enum after Struct = 8
    Error = 9,
```

- [ ] **Step 3: Add `SlotData::Error` variant**

```rust
// src/runtime/runtime.rs, in the SlotData enum before Empty
    Error {
        code: u32,
        message: Option<String>,
    },
```

- [ ] **Step 4: Add `Tag::Error` to `is_heap_tag`**

```rust
// src/runtime/runtime.rs, in is_heap_tag, add Error to the matches
const fn is_heap_tag(tag: i32) -> bool {
    matches!(
        tag,
        t if t == Tag::String as i32
            || t == Tag::List as i32
            || t == Tag::Range as i32
            || t == Tag::Struct as i32
            || t == Tag::Enum as i32
            || t == Tag::Error as i32
    )
}
```

- [ ] **Step 5: Add `Drop` handling for `SlotData::Error`**

```rust
// src/runtime/runtime.rs, in the Drop impl for SlotData
// Add Error to the no-op drop arm alongside List/String/Range
            SlotData::List(_) | SlotData::String(_) | SlotData::Range(_)
            | SlotData::Error { .. } => {}
```

- [ ] **Step 6: Build and verify**

Run: `cargo build`
Expected: Build succeeds

- [ ] **Step 7: Commit**

```bash
git add src/runtime/runtime.rs src/llvm/compiler.rs
git commit -m "feat: add Tag::Error and SlotData::Error for catchable error mechanism"
```

---

## Task 2: ランタイム関数の追加（__error_new, __is_error, __error_code, __error_message）

**Files:**
- Modify: `src/runtime/runtime.rs` (add 4 new `extern "C"` functions after `__clone`)

**Interfaces:**
- Produces: `__error_new(code: u32, message_ptr: *const u8, message_len: u64) -> u64` — creates a `SlotData::Error` slot and returns its handle.
- Produces: `__is_error(handle: u64) -> i32` — returns 1 if the slot is `Tag::Error`, 0 otherwise.
- Produces: `__error_code(handle: u64) -> u32` — returns the error code, 0 if not an error.
- Produces: `__error_message(handle: u64) -> u64` — returns a String slab handle for the message, `INVALID_HANDLE` if not an error or no message.

- [ ] **Step 1: Add `__error_new` function**

```rust
// src/runtime/runtime.rs, after __clone function (line ~615)

/// Create an error value in the slab. `message_ptr` may be null (no message).
#[unsafe(no_mangle)]
pub extern "C" fn __error_new(code: u32, message_ptr: *const u8, message_len: u64) -> u64 {
    let message = if message_ptr.is_null() || message_len == 0 {
        None
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(message_ptr, message_len as usize) };
        Some(String::from_utf8_lossy(bytes).into_owned())
    };
    slot_insert(SlotData::Error { code, message })
}
```

- [ ] **Step 2: Add `__is_error` function**

```rust
/// Check if a value's tag is `Tag::Error`. Returns 1 (true) or 0 (false).
#[unsafe(no_mangle)]
pub extern "C" fn __is_error(handle: u64) -> i32 {
    let tag = slot_with(handle, Tag::Unit as i32, |d| {
        // The tag for Error is known; we check if the slot is Error.
        if matches!(d, SlotData::Error { .. }) {
            Tag::Error as i32
        } else {
            Tag::Unit as i32
        }
    });
    if tag == Tag::Error as i32 { 1 } else { 0 }
}
```

Note: `__is_error` takes a `SprsValue`'s `data` field (the slab handle). The caller passes `val.data`. The function checks whether the slot at that handle is `SlotData::Error`.

- [ ] **Step 3: Add `__error_code` function**

```rust
/// Get the error code from an error value. Returns 0 if not an error.
#[unsafe(no_mangle)]
pub extern "C" fn __error_code(handle: u64) -> u32 {
    slot_with(handle, 0u32, |d| match d {
        SlotData::Error { code, .. } => *code,
        _ => 0,
    })
}
```

- [ ] **Step 4: Add `__error_message` function**

```rust
/// Get the error message as a new String slab handle.
/// Returns INVALID_HANDLE if not an error or no message.
#[unsafe(no_mangle)]
pub extern "C" fn __error_message(handle: u64) -> u64 {
    let message_opt: Option<String> = slot_with(handle, None, |d| match d {
        SlotData::Error { message, .. } => message.clone(),
        _ => None,
    });
    match message_opt {
        Some(s) => slot_insert(SlotData::String(s)),
        None => INVALID_HANDLE,
    }
}
```

- [ ] **Step 5: Add `Tag::Error` display in `format_sprs_value`**

```rust
// src/runtime/runtime.rs, in format_sprs_value, before the `_ =>` arm
        t if t == Tag::Error as i32 => {
            let (code, msg) = slot_with(val.data, (0u32, String::new()), |d| match d {
                SlotData::Error { code, message } => {
                    (*code, message.clone().unwrap_or_default())
                }
                _ => (0, String::new()),
            });
            use std::fmt::Write;
            if msg.is_empty() {
                let _ = write!(out, "<error code={}>", code);
            } else {
                let _ = write!(out, "<error code={} \"{}\">", code, msg);
            }
        }
```

- [ ] **Step 6: Build and verify**

Run: `cargo build`
Expected: Build succeeds

- [ ] **Step 7: Commit**

```bash
git add src/runtime/runtime.rs
git commit -m "feat: add __error_new, __is_error, __error_code, __error_message runtime functions"
```

---

## Task 3: get_runtime_fn への新関数登録

**Files:**
- Modify: `src/llvm/compiler.rs:466-537` (get_runtime_fn match arms)

**Interfaces:**
- Produces: `get_runtime_fn` recognizes `__error_new`, `__is_error`, `__error_code`, `__error_message` and returns the correct LLVM function types.

- [ ] **Step 1: Add match arms for the 4 new runtime functions**

```rust
// src/llvm/compiler.rs, in get_runtime_fn, before the "__panic" arm

            "__error_new" => i64_type.fn_type(
                &[
                    i32_type.into(),       // error code
                    i8_ptr_type.into(),    // message ptr (may be null)
                    i64_type.into(),       // message length
                ],
                false,
            ),
            "__is_error" => i32_type.fn_type(
                &[i64_type.into()],       // slab handle (data field)
                false,
            ),
            "__error_code" => i32_type.fn_type(
                &[i64_type.into()],       // slab handle
                false,
            ),
            "__error_message" => i64_type.fn_type(
                &[i64_type.into()],       // slab handle
                false,
            ),
```

- [ ] **Step 2: Build and verify**

Run: `cargo build`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add src/llvm/compiler.rs
git commit -m "feat: register error runtime functions in get_runtime_fn"
```

---

## Task 4: create_error_value（create_panic_err の置換）

**Files:**
- Modify: `src/llvm/value.rs:16-51` (PanicErrorSettings → ErrorValueSettings, create_panic_err → create_error_value)

**Interfaces:**
- Produces: `create_error_value(compiler, error_code: u32, message: &str, module) -> Result<PointerValue, SprsError>` — generates IR that calls `__error_new`, stores the result as `Tag::Error` in a `runtime_value_type` alloca, and returns the pointer. The caller no longer needs `build_unreachable`.
- Note: `PanicErrorSettings` struct at `value.rs:16` is renamed to `ErrorValueSettings` but the fields `is_const` and `is_global` remain the same (they control `set_global_constant_str`).

- [ ] **Step 1: Rename `PanicErrorSettings` to `ErrorValueSettings`**

```rust
// src/llvm/value.rs, line 16
pub struct ErrorValueSettings {
    pub is_const: bool,
    pub is_global: bool,
}
```

- [ ] **Step 2: Replace `create_panic_err` with `create_error_value`**

```rust
// src/llvm/value.rs, replacing the create_panic_err function body

/// Generate IR that creates a `Tag::Error` value in the slab.
/// Stores the result as a runtime_value_type `{ i32 tag=Error, i64 data=handle }`
/// in a fresh alloca and returns the pointer.
/// The caller should NOT emit `build_unreachable` — the error value flows
/// through normal control flow so callers can propagate or catch it.
pub fn create_error_value<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    error_code: u32,
    message: &str,
    module: &inkwell::module::Module<'ctx>,
    settings: ErrorValueSettings,
) -> Result<PointerValue<'ctx>, SprsError> {
    // Store the message string as a global constant.
    let global = self_compiler.set_global_constant_str(
        module,
        message,
        settings.is_global,
        settings.is_const,
    );

    let (msg_ptr, msg_len) = match global {
        Some(StrConstantResult::Global(g)) => {
            let ptr = g.as_pointer_value();
            let ptr_i8 = self_compiler.builder.build_bit_cast(
                ptr,
                self_compiler.context.ptr_type(AddressSpace::default()),
                "error_msg_ptr_i8",
            );
            (ptr_i8.unwrap().into_pointer_value(), message.len() as u64)
        }
        Some(StrConstantResult::Pointer(p)) => {
            (p, message.len() as u64)
        }
        None => {
            // Empty message — pass null pointer.
            let null_ptr = self_compiler.context.ptr_type(AddressSpace::default()).const_null();
            (null_ptr, 0u64)
        }
    };

    let error_code_val = self_compiler.context.i32_type().const_int(error_code as u64, false);
    let msg_len_val = self_compiler.context.i64_type().const_int(msg_len, false);

    let error_new_fn = self_compiler.get_runtime_fn(module, "__error_new")?;
    let error_handle = self_compiler
        .builder
        .build_call(error_new_fn, &[error_code_val.into(), msg_ptr.into(), msg_len_val.into()], "error_new_call")
        .unwrap()
        .try_as_basic_value()
        .into_int_value();

    // Store as a runtime_value_type { tag: Error, data: handle }
    let res_ptr = create_entry_block_alloca(self_compiler, "error_val")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Error as u64),
        StoreValue::Int(error_handle),
        "error_val_store",
    );

    Ok(res_ptr)
}
```

- [ ] **Step 3: Update all imports of `PanicErrorSettings` and `create_panic_err`**

The following files import `PanicErrorSettings` and `create_panic_err`:
- `src/llvm/arithmetic.rs:13`
- `src/llvm/control_flow.rs:11`
- `src/llvm/data_structures.rs:11`
- `src/llvm/macros.rs:11`

Change each import from:
```rust
use crate::llvm::value::{create_panic_err, create_entry_block_alloca, PanicErrorSettings};
```
to:
```rust
use crate::llvm::value::{create_error_value, create_entry_block_alloca, ErrorValueSettings};
```

Note: `control_flow.rs` and `data_structures.rs` import `create_panic_err` but are dead imports (not actually called). Remove `create_panic_err` from their imports entirely; keep only what they use.

- [ ] **Step 4: Build (expect errors at call sites — they'll be fixed in Task 5)**

Run: `cargo build`
Expected: Build errors at the 6 `create_panic_err` call sites. This is expected — Task 5 fixes them.

- [ ] **Step 5: Commit**

```bash
git add src/llvm/value.rs src/llvm/arithmetic.rs src/llvm/control_flow.rs src/llvm/data_structures.rs src/llvm/macros.rs
git commit -m "feat: replace create_panic_err with create_error_value"
```

---

## Task 5: 6箇所の panic サイトを create_error_value に置換

**Files:**
- Modify: `src/llvm/arithmetic.rs:110-131` (add type mismatch)
- Modify: `src/llvm/arithmetic.rs:612-625` (add float tag error)
- Modify: `src/llvm/arithmetic.rs:1602-1609` (div by zero)
- Modify: `src/llvm/arithmetic.rs:1680-1687` (mod by zero)
- Modify: `src/llvm/macros.rs:290-302` (cast type error)
- Modify: `src/llvm/macros.rs:803-822` (shift type error)

**Interfaces:**
- Consumes: `create_error_value` from Task 4
- Produces: All 6 panic sites replaced with `create_error_value` calls. Each site stores the error value and branches to the merge block (instead of `build_unreachable`).

- [ ] **Step 1: Replace arithmetic.rs:110-131 (add type mismatch)**

Replace the error branch in `create_add_expr`:

```rust
// src/llvm/arithmetic.rs, in create_add_expr error_bb (was lines 110-131)

    // error branch: create a TypeMismatch error value (no __panic, no unreachable)
    self_compiler.builder.position_at_end(error_bb);

    let settings = ErrorValueSettings {
        is_const: true,
        is_global: true,
    };

    let error_ptr = create_error_value(
        self_compiler,
        4,  // SprsErrorCode::TypeMismatch
        "TypeError: type mismatch in add",
        module,
        settings,
    )?;

    // Store the error pointer as the result and branch to merge.
    // The merge PHI will pick this up.
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);
```

Important: The `error_bb` now branches to `merge_bb` instead of ending with `build_unreachable`. The merge PHI needs to account for this incoming path. Check the existing merge PHI in `create_add_expr` — it currently only has incoming from `int_bb`, `float_bb`, and `string_bb`. Add the `error_bb` → `merge_bb` incoming.

Examine the merge PHI at the end of `create_add_expr` and add the error_ptr as an incoming value from `error_bb`.

- [ ] **Step 2: Replace arithmetic.rs:612-625 (float tag error)**

Same pattern — replace `create_panic_err` + `build_unreachable` with `create_error_value` + `build_unconditional_branch(merge_bb)`. Error code: `4` (TypeMismatch).

- [ ] **Step 3: Replace arithmetic.rs:1602-1609 (div by zero)**

```rust
// src/llvm/arithmetic.rs, in create_div_expr bb_err

    self_compiler.builder.position_at_end(bb_err);
    let settings = ErrorValueSettings {
        is_const: true,
        is_global: true,
    };
    let error_ptr = create_error_value(
        self_compiler,
        2,  // SprsErrorCode::DivByZero
        "Division by zero",
        module,
        settings,
    )?;
    // Return the error value directly — div/mod functions return a pointer.
    return Ok(error_ptr.into());
```

Note: `create_div_expr` and `create_mod_expr` return `Result<BasicValueEnum, SprsError>` directly (they don't have a merge block). So the error value is returned directly via `return Ok(error_ptr.into())`.

- [ ] **Step 4: Replace arithmetic.rs:1680-1687 (mod by zero)**

Same as Step 3 but with error code `3` (ModByZero) and message `"Modulo by zero"`.

- [ ] **Step 5: Replace macros.rs:290-302 (cast type error)**

```rust
// src/llvm/macros.rs, in call_builtin_macro_cast error_bb

    self_compiler.builder.position_at_end(error_bb);
    let settings = ErrorValueSettings {
        is_const: true,
        is_global: true,
    };
    let error_ptr = create_error_value(
        self_compiler,
        5,  // SprsErrorCode::CastError
        "TypeError: unexpected tag in @cast",
        module,
        settings,
    )?;
    return Ok(error_ptr.into());
```

- [ ] **Step 6: Replace macros.rs:803-822 (shift type error)**

```rust
// src/llvm/macros.rs, in shift macro bb_err

    self_compiler.builder.position_at_end(bb_err);
    let error_msg: &str = match dir {
        ShiftDir::Left => "@lshift expects an integer value",
        ShiftDir::Right => "@rshift expects an integer value",
    };
    let settings = ErrorValueSettings {
        is_const: true,
        is_global: true,
    };
    let error_ptr = create_error_value(
        self_compiler,
        6,  // SprsErrorCode::ShiftTypeError
        error_msg,
        module,
        settings,
    )?;
    return Ok(error_ptr.into());
```

- [ ] **Step 7: Build and fix any PHI incoming issues**

Run: `cargo build`
Expected: Build may fail if merge PHI nodes don't account for the new `error_bb` incoming. Fix by adding the error_ptr as an incoming value in each merge PHI.

The key change: previously `error_bb` was a dead block (ended with `unreachable`, never branched to merge). Now it branches to `merge_bb`, so the PHI must have an incoming from `error_bb`.

- [ ] **Step 8: Run existing tests**

Run: `cargo test`
Expected: All existing tests pass (87 PASS / 7 XFAIL / 8 FAIL maintained). The error values now flow through normal control flow instead of calling `__panic`.

- [ ] **Step 9: Commit**

```bash
git add src/llvm/arithmetic.rs src/llvm/macros.rs
git commit -m "feat: replace 6 panic sites with create_error_value"
```

---

## Task 6: __clone に Tag::Error 分岐を追加

**Files:**
- Modify: `src/runtime/runtime.rs:564-615` (__clone function)

**Interfaces:**
- Produces: `__clone` handles `Tag::Error` by cloning the `SlotData::Error` slot.

- [ ] **Step 1: Add `Tag::Error` clone branch**

```rust
// src/runtime/runtime.rs, in __clone, after the Enum branch (line ~608)

    if tag == Tag::Error as i32 {
        let new_handle = error_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }
```

- [ ] **Step 2: Add the `error_clone` helper function**

```rust
// src/runtime/runtime.rs, after enum_clone (line ~709)

fn error_clone(handle: u64) -> u64 {
    let (code, message) = slot_with(handle, (0u32, None), |d| match d {
        SlotData::Error { code, message } => (*code, message.clone()),
        _ => (0, None),
    });
    slot_insert(SlotData::Error { code, message })
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo build`
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add src/runtime/runtime.rs
git commit -m "feat: add Tag::Error clone support"
```

---

## Task 7: short-circuit ルールの追加

**Files:**
- Modify: `src/llvm/arithmetic.rs` (create_add_expr, create_minus_expr, create_mul_expr, create_div_expr, create_mod_expr)
- Modify: `src/llvm/macros.rs` (call_builtin_macro_cast, call_builtin_macro_lshift, call_builtin_macro_rshift)

**Interfaces:**
- Produces: All tag-dispatch sites check for `Tag::Error` operands before any other tag comparison. If either operand is `Tag::Error`, that operand's value is returned directly (no new error generated).

- [ ] **Step 1: Add short-circuit to create_add_expr**

In `create_add_expr`, after loading `l_tag` and `r_tag` (after line 51), add a short-circuit check before the existing tag dispatch:

```rust
// src/llvm/arithmetic.rs, in create_add_expr, after r_tag is loaded

    // short-circuit: if either operand is Tag::Error, return it directly.
    let error_tag_const = self_compiler.context.i32_type().const_int(Tag::Error as u64, false);
    let l_is_error = self_compiler.builder.build_int_compare(
        inkwell::IntPredicate::EQ, l_tag, error_tag_const, "l_is_error"
    ).unwrap();
    let l_error_bb = self_compiler.context.append_basic_block(parent_fn, "l_error_short_circuit");
    let check_r_error_bb = self_compiler.context.append_basic_block(parent_fn, "check_r_error");
    let _ = self_compiler.builder.build_conditional_branch(l_is_error, l_error_bb, check_r_error_bb);

    // l is error → return l_ptr
    self_compiler.builder.position_at_end(l_error_bb);
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    // check r
    self_compiler.builder.position_at_end(check_r_error_bb);
    let r_is_error = self_compiler.builder.build_int_compare(
        inkwell::IntPredicate::EQ, r_tag, error_tag_const, "r_is_error"
    ).unwrap();
    let r_error_bb = self_compiler.context.append_basic_block(parent_fn, "r_error_short_circuit");
    let normal_dispatch_bb = self_compiler.context.append_basic_block(parent_fn, "normal_dispatch");
    let _ = self_compiler.builder.build_conditional_branch(r_is_error, r_error_bb, normal_dispatch_bb);

    // r is error → return r_ptr
    self_compiler.builder.position_at_end(r_error_bb);
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    // Normal dispatch continues here
    self_compiler.builder.position_at_end(normal_dispatch_bb);
```

Then the existing `can_add` check and subsequent branches continue from `normal_dispatch_bb` instead of the original position.

The merge PHI must add incoming from `l_error_bb` (value: `l_ptr`) and `r_error_bb` (value: `r_ptr`).

- [ ] **Step 2: Repeat for create_minus_expr, create_mul_expr**

Apply the same short-circuit pattern to `create_minus_expr` and `create_mul_expr`. These have the same structure as `create_add_expr` (load l_tag, r_tag, dispatch by tag).

- [ ] **Step 3: Repeat for create_div_expr, create_mod_expr**

These functions take `IntValue` parameters (not pointers), so the short-circuit is different: they receive already-loaded tag values. Check if the tag is `Tag::Error` at the function entry and return early.

Examine the function signatures: `create_div_expr` and `create_mod_expr` take `(compiler, l_val: IntValue, r_val: IntValue, l_tag: IntValue, r_tag: IntValue, module)`. Add at the top:

```rust
    // short-circuit: if either operand is Error, return it as a runtime_value
    let error_tag_const = self_compiler.context.i32_type().const_int(Tag::Error as u64, false);
    let l_is_error = self_compiler.builder.build_int_compare(
        inkwell::IntPredicate::EQ, l_tag, error_tag_const, "div_l_is_error"
    ).unwrap();
```

Note: `create_div_expr` and `create_mod_expr` return `Result<BasicValueEnum, SprsError>`. If an operand is `Tag::Error`, we need to return the error value. But these functions receive `IntValue` (the data) not `PointerValue` (the full runtime value). The caller (`compile_expr` for `Expr::Div`/`Expr::Mod`) has the full pointers. The short-circuit for div/mod must be done at the caller level (in `compile_expr`) before calling `create_div_expr`/`create_mod_expr`.

Alternative: Change `create_div_expr`/`create_mod_expr` to take `PointerValue` instead of `IntValue`. This is a larger change. The simpler approach: add the short-circuit in `compile_expr` at the `Expr::Div`/`Expr::Mod` match arms, before calling the arithmetic functions.

- [ ] **Step 4: Add short-circuit for @cast, @lshift, @rshift**

In `call_builtin_macro_cast`, after loading the input value's tag, add:

```rust
    // short-circuit: if input is Error, return it directly
    let error_tag_const = self_compiler.context.i32_type().const_int(Tag::Error as u64, false);
    let input_is_error = self_compiler.builder.build_int_compare(
        inkwell::IntPredicate::EQ, current_tag, error_tag_const, "cast_input_is_error"
    ).unwrap();
    let input_error_bb = self_compiler.context.append_basic_block(parent_fn, "cast_input_error");
    let cast_normal_bb = self_compiler.context.append_basic_block(parent_fn, "cast_normal");
    let _ = self_compiler.builder.build_conditional_branch(input_is_error, input_error_bb, cast_normal_bb);

    self_compiler.builder.position_at_end(input_error_bb);
    // Return the input pointer directly
    // (cast returns a pointer to runtime_value_type)
    let _ = self_compiler.builder.build_unconditional_branch(merge_bb);

    self_compiler.builder.position_at_end(cast_normal_bb);
    // ... existing switch continues here
```

Same pattern for `@lshift` and `@rshift`.

- [ ] **Step 5: Build and run tests**

Run: `cargo build && cargo test`
Expected: Build succeeds, all existing tests pass

- [ ] **Step 6: Commit**

```bash
git add src/llvm/arithmetic.rs src/llvm/macros.rs src/llvm/codegen.rs
git commit -m "feat: add error short-circuit to all tag-dispatch sites"
```

---

## Task 8: ? 演算子 — lexer と AST

**Files:**
- Modify: `src/front/lexer.rs:71-206` (RawTok enum, Token enum, RawTok→Token mapping)
- Modify: `src/front/ast.rs:5-49` (Expr enum)
- Modify: `src/grammar.lalrpop:393-421` (Postfix rule)

**Interfaces:**
- Produces: `Token::Question` in lexer. `Expr::Try(Box<Spanned<Expr>>)` in AST. Grammar rule: `<base:Postfix> Question => Expr::Try(Box::new(base))`.

- [ ] **Step 1: Add `Question` token to RawTok**

```rust
// src/front/lexer.rs, in the RawTok enum, after GtGt
    #[token("?")]
    Question,
```

- [ ] **Step 2: Add `Question` to the Token enum and RawTok→Token mapping**

In the `Token` enum (if separate from RawTok) and in the `next()` method's match:

```rust
// In the match from RawTok to Token
RawTok::Question => Token::Question,
```

Examine the existing code: `src/front/lexer.rs:231` has the `match res` block. Add the Question mapping there.

- [ ] **Step 3: Add `Expr::Try` variant to AST**

```rust
// src/front/ast.rs, in the Expr enum, after StructInit
    Try(Box<Spanned<Expr>>),                    // Error propagation: expr?
```

- [ ] **Step 4: Add grammar rule for `?`**

```lalrpop
// src/grammar.lalrpop, in the Postfix rule, add before <f:Atom> => f
    <start:@L> <base:Postfix> Question <end:@R> => Spanned::new(Expr::Try(Box::new(base)), Span::new(start, end)),
```

- [ ] **Step 5: Add `Question` to the Token mapping in grammar.lalrpop**

```lalrpop
// src/grammar.lalrpop, in the extern Token block
    Question => Token::Question,
```

- [ ] **Step 6: Build and verify**

Run: `cargo build`
Expected: Build succeeds (codegen doesn't handle `Expr::Try` yet — that's Task 9)

- [ ] **Step 7: Commit**

```bash
git add src/front/lexer.rs src/front/ast.rs src/grammar.lalrpop
git commit -m "feat: add ? token, Expr::Try AST node, and grammar rule"
```

---

## Task 9: ? 演算子の codegen

**Files:**
- Modify: `src/llvm/codegen.rs:455-674` (compile_expr, add Expr::Try arm)

**Interfaces:**
- Consumes: `Expr::Try` from Task 8, `emit_drop_for_return` from compiler.rs

- [ ] **Step 1: Add `Expr::Try` arm to `compile_expr`**

```rust
// src/llvm/codegen.rs, in compile_expr match, add
            ast::Expr::Try(inner_expr) => {
                let inner_ptr = self.compile_expr(inner_expr, module)?.into_pointer_value();

                // Load the tag of the inner result.
                let tag_ptr = self
                    .builder
                    .build_struct_gep(self.runtime_value_type, inner_ptr, 0, "try_tag_ptr")
                    .unwrap();
                let tag_val = self
                    .builder
                    .build_load(self.context.i32_type(), tag_ptr, "try_tag")
                    .unwrap()
                    .into_int_value();

                let error_tag_const = self.context.i32_type().const_int(Tag::Error as u64, false);
                let is_error = self
                    .builder
                    .build_int_compare(inkwell::IntPredicate::EQ, tag_val, error_tag_const, "try_is_error")
                    .unwrap();

                let current_fn = self.function_signatures.unwrap();
                let propagate_bb = self.context.append_basic_block(current_fn, "try_propagate");
                let continue_bb = self.context.append_basic_block(current_fn, "try_continue");

                let _ = self
                    .builder
                    .build_conditional_branch(is_error, propagate_bb, continue_bb);

                // Propagate: emit drops and return the error value.
                self.builder.position_at_end(propagate_bb);
                self.emit_drop_for_return(module)?;
                self.builder.build_return(Some(&inner_ptr.into())).unwrap();

                // Continue: the inner value is not an error, use it.
                self.builder.position_at_end(continue_bb);
                Ok(inner_ptr.into())
            }
```

- [ ] **Step 2: Build and verify**

Run: `cargo build`
Expected: Build succeeds

- [ ] **Step 3: Run existing tests**

Run: `cargo test`
Expected: All existing tests pass

- [ ] **Step 4: Commit**

```bash
git add src/llvm/codegen.rs
git commit -m "feat: implement ? operator codegen for error propagation"
```

---

## Task 10: エラーマクロの codegen（@is_error, @error_code, @error_message, @error）

**Files:**
- Modify: `src/llvm/codegen.rs:489-510` (Macro dispatch in compile_expr)
- Modify: `src/llvm/macros.rs` (add 4 new macro functions)

**Interfaces:**
- Consumes: `__is_error`, `__error_code`, `__error_message`, `__error_new` runtime functions from Task 2
- Produces: `@is_error(x)` returns bool, `@error_code(x)` returns i64, `@error_message(x)` returns String handle, `@error(code, message)` returns error value.

- [ ] **Step 1: Add macro dispatch entries in compile_expr**

```rust
// src/llvm/codegen.rs, in the Expr::Macro match, add
                    "is_error" => Ok(builder_helper::call_builtin_macro_is_error(self, args, module)?),
                    "error_code" => Ok(builder_helper::call_builtin_macro_error_code(self, args, module)?),
                    "error_message" => Ok(builder_helper::call_builtin_macro_error_message(self, args, module)?),
                    "error" => Ok(builder_helper::call_builtin_macro_error(self, args, module)?),
```

- [ ] **Step 2: Implement `call_builtin_macro_is_error`**

```rust
// src/llvm/macros.rs

/// @is_error(x) — returns true (1) if x's tag is Tag::Error, false (0) otherwise.
pub fn call_builtin_macro_is_error<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@is_error expects exactly 1 argument".to_string(),
            help: None,
        });
    }

    let val_ptr = self_compiler.compile_expr(&args[0], module)?.into_pointer_value();

    // Load the data field (slab handle) from the runtime_value.
    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 1, "is_error_data_ptr")
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "is_error_data")
        .unwrap()
        .into_int_value();

    let is_error_fn = self_compiler.get_runtime_fn(module, "__is_error")?;
    let result = self_compiler
        .builder
        .build_call(is_error_fn, &[data_val.into()], "is_error_call")
        .unwrap()
        .try_as_basic_value()
        .into_int_value();

    // Store as a Bool runtime_value.
    let res_ptr = create_entry_block_alloca(self_compiler, "is_error_res")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Boolean as u64),
        StoreValue::Int(result),
        "is_error_res_store",
    );
    Ok(res_ptr.into())
}
```

- [ ] **Step 3: Implement `call_builtin_macro_error_code`**

```rust
/// @error_code(x) — returns the error code as an i64. Returns 0 if not an error.
pub fn call_builtin_macro_error_code<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@error_code expects exactly 1 argument".to_string(),
            help: None,
        });
    }

    let val_ptr = self_compiler.compile_expr(&args[0], module)?.into_pointer_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 1, "error_code_data_ptr")
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "error_code_data")
        .unwrap()
        .into_int_value();

    let error_code_fn = self_compiler.get_runtime_fn(module, "__error_code")?;
    let code_i32 = self_compiler
        .builder
        .build_call(error_code_fn, &[data_val.into()], "error_code_call")
        .unwrap()
        .try_as_basic_value()
        .into_int_value();

    // Zero-extend i32 to i64 for the data field.
    let code_i64 = self_compiler
        .builder
        .build_int_z_extend(code_i32, self_compiler.context.i64_type(), "error_code_i64")
        .unwrap();

    let res_ptr = create_entry_block_alloca(self_compiler, "error_code_res")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::Integer as u64),
        StoreValue::Int(code_i64),
        "error_code_res_store",
    );
    Ok(res_ptr.into())
}
```

- [ ] **Step 4: Implement `call_builtin_macro_error_message`**

```rust
/// @error_message(x) — returns the error message as a String value.
/// Returns an empty string if not an error or no message.
pub fn call_builtin_macro_error_message<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 1 {
        return Err(SprsError::Semantic {
            code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@error_message expects exactly 1 argument".to_string(),
            help: None,
        });
    }

    let val_ptr = self_compiler.compile_expr(&args[0], module)?.into_pointer_value();

    let data_ptr = self_compiler
        .builder
        .build_struct_gep(self_compiler.runtime_value_type, val_ptr, 1, "error_msg_data_ptr")
        .unwrap();
    let data_val = self_compiler
        .builder
        .build_load(self_compiler.context.i64_type(), data_ptr, "error_msg_data")
        .unwrap()
        .into_int_value();

    let error_msg_fn = self_compiler.get_runtime_fn(module, "__error_message")?;
    let string_handle = self_compiler
        .builder
        .build_call(error_msg_fn, &[data_val.into()], "error_msg_call")
        .unwrap()
        .try_as_basic_value()
        .into_int_value();

    // Store as a String runtime_value.
    let res_ptr = create_entry_block_alloca(self_compiler, "error_msg_res")?;
    self_compiler.build_runtime_value_store(
        res_ptr,
        StoreTag::Int(Tag::String as u64),
        StoreValue::Int(string_handle),
        "error_msg_res_store",
    );
    Ok(res_ptr.into())
}
```

- [ ] **Step 5: Implement `call_builtin_macro_error`**

```rust
/// @error(code, message) — creates a Tag::Error value.
/// code: integer literal (u32). message: string literal.
pub fn call_builtin_macro_error<'ctx>(
    self_compiler: &mut Compiler<'ctx>,
    args: &[Spanned<ast::Expr>],
    module: &inkwell::module::Module<'ctx>,
) -> Result<BasicValueEnum<'ctx>, SprsError> {
    if args.len() != 2 {
        return Err(SprsError::Semantic {
            code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
            location: Location::new(String::new(), Span::DUMMY),
            message: "@error expects exactly 2 arguments: code and message".to_string(),
            help: None,
        });
    }

    // Extract code from the first argument (must be a Number literal).
    let error_code: u32 = match &args[0].node {
        ast::Expr::Number(n) => *n as u32,
        _ => return Err(SprsError::Semantic {
            code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
            location: Location::new(String::new(), args[0].span),
            message: "@error first argument must be an integer literal".to_string(),
            help: None,
        }),
    };

    // Extract message from the second argument (must be a Str literal).
    let message: &str = match &args[1].node {
        ast::Expr::Str(s) => s.as_str(),
        _ => return Err(SprsError::Semantic {
            code: ErrorCode { category: ErrorCategory::Semantic, number: 3 },
            location: Location::new(String::new(), args[1].span),
            message: "@error second argument must be a string literal".to_string(),
            help: None,
        }),
    };

    let settings = ErrorValueSettings {
        is_const: true,
        is_global: true,
    };

    let error_ptr = create_error_value(self_compiler, error_code, message, module, settings)?;
    Ok(error_ptr.into())
}
```

- [ ] **Step 6: Build and verify**

Run: `cargo build`
Expected: Build succeeds

- [ ] **Step 7: Run existing tests**

Run: `cargo test`
Expected: All existing tests pass

- [ ] **Step 8: Commit**

```bash
git add src/llvm/codegen.rs src/llvm/macros.rs
git commit -m "feat: implement @is_error, @error_code, @error_message, @error macros"
```

---

## Task 11: main 境界の __panic とテスト追加

**Files:**
- Modify: `src/llvm/codegen.rs` or `src/llvm/compiler.rs` (main function codegen)
- Modify: `tests/src/main.sprs` (add error mechanism test cases)

**Interfaces:**
- Produces: `main` function checks if the return value is `Tag::Error` and calls `__panic` if so. Test cases for error catch, propagation, short-circuit, and user-defined errors.

- [ ] **Step 1: Add main-level error check**

In the codegen for the `main` function (or the wrapper that calls `main`), after the call returns, check if the result's tag is `Tag::Error`. If so, call `__panic` with the error message.

Examine how `main` is currently compiled. The `compile_fn` at `codegen.rs:119` handles all functions including `main`. The `main` function returns `void` currently (or a runtime value). After `main` returns at the top level, add a tag check:

```rust
// After main's return value is available, check if it's an error.
// If the main function returns a runtime_value_type, load its tag.
// If tag == Tag::Error, call __panic with the error message.
```

Note: sprs `main` currently takes no arguments and may return `()` or a value. The error check should be added at the call site in `llvm_executer.rs` or the IR that calls `main`. Examine the existing main invocation pattern.

If `main` returns `void`, no check is needed at the main level — errors would only surface if they're caught by `@is_error` or propagated via `?`. Uncatched errors in expressions would short-circuit through operations and eventually be printed via `@println` (which calls `format_sprs_value` and shows `<error code=N>`).

For the case where `main` returns a value: add a check after the main call. If the return is `Tag::Error`, call `__panic`.

- [ ] **Step 2: Add test cases to tests/src/main.sprs**

Add test cases under an "Error Mechanism" section:

```sprs
# === Error Mechanism ===

# Test: @error creates a catchable error
fn make_error() >> i64 {
    return @error(100, "test error");
}

# Test: @is_error detects errors
fn check_error() >> bool {
    var x = make_error();
    return @is_error(x);
}

# Test: @error_code extracts the code
fn get_error_code() >> i64 {
    var x = make_error();
    return @error_code(x);
}

# Test: @error_message extracts the message
fn get_error_message() >> str {
    var x = make_error();
    return @error_message(x);
}

# Test: ? propagation
fn propagate_error() >> i64 {
    var x = make_error()?;
    return x + 1;
}

# Test: short-circuit (error + 1 should preserve the error)
fn short_circuit_test() >> bool {
    var x = make_error();
    var y = x + 1;
    return @is_error(y);
}
```

- [ ] **Step 3: Run tests and verify**

Run: `cargo test`
Expected: New tests pass. Existing tests maintain their PASS/XFAIL/FAIL status.

- [ ] **Step 4: Commit**

```bash
git add src/llvm/codegen.rs tests/src/main.sprs
git commit -m "feat: add main-level error check and error mechanism tests"
```

---

## Task 12: BUG_REPORT.md 更新と最終確認

**Files:**
- Modify: `BUG_REPORT.md`
- Modify: `docs/superpowers/specs/2026-07-29-catchable-error-mechanism-design.md` (status → Implemented)

- [ ] **Step 1: Update BUG_REPORT.md**

Add the catchable error mechanism to the resolved bugs section. Update BUG-L04 and BUG-L06 references to point to the new mechanism.

- [ ] **Step 2: Update spec status**

Change the spec status from "Approved" to "Implemented".

- [ ] **Step 3: Final full test run**

Run: `cargo test`
Expected: All tests pass (existing + new error mechanism tests)

- [ ] **Step 4: Commit**

```bash
git add BUG_REPORT.md docs/superpowers/specs/2026-07-29-catchable-error-mechanism-design.md
git commit -m "docs: update BUG_REPORT and spec status for catchable error mechanism"
```

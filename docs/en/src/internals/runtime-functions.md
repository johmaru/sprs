# Runtime Functions

These symbols are compiler and runtime internal APIs. They are not language built-ins.

Do not call these `__` symbols from Sprs source. They are not language-level APIs.

A runtime value is `{ i32 tag, i64 data }`. This is the evaluator representation. It is not pointer storage and is not the source of truth for size, alignment, or `Ptr(T) + n` stride.

Typed pointers address `StorageRep(T)`: the LLVM concrete type plus the target ABI size and alignment. `StorageRep(MaybeUninit(T))` is identical to `StorageRep(T)`. Struct `StorageRep` is an inline field layout (padding included). Runtime-managed owned types store a slab handle in that layout; the payload stays on the slab.

For heap values, RuntimeValue `data` is a handle `(index:u32 << 32) | generation:u32`. Handle `0` is invalid. Atom `data` is an intern id. RawPtr `data` is a bare address.

| Tag | Value |
|-----|-------|
| Integer | 0 |
| Float | 1 |
| String | 2 |
| Boolean | 3 |
| List | 4 |
| Range | 5 |
| Unit | 6 |
| (unused) | 7 |
| Struct | 8 |
| Atom | 9 |
| Label | 10 |
| Buffer | 11 |
| RawPtr | 12 |
| Int8 | 100 |
| Uint8 | 101 |
| Int16 | 102 |
| Uint16 | 103 |
| Int32 | 104 |
| Uint32 | 105 |
| Int64 | 106 |
| Uint64 | 107 |
| Float16 | 108 |
| Float32 | 109 |
| Float64 | 110 |

| Function Name   | Description                          |
|-----------------|--------------------------------------|
| __list_new | for creating a new list|
| __list_get | destructive take of a list element; leaves Unit in that slot|
| __list_push | for pushing an element to the end of a list|
| __list_set | replace list element at index; OOB / bad handle drops the incoming value|
| __range_new | for creating a new range|
| __println | for printing values to the console|
| __strlen | for getting the length of a string|
| __malloc | for allocating memory|
| __drop | for dropping a value|
| __clone | for cloning a value|
| __panic | for handling panic situations|
| __buffer_new | allocate a Buffer |
| __buffer_len | Buffer length |
| __buffer_get | Buffer byte read |
| __buffer_set | Buffer byte write |
| __buffer_exist | Buffer liveness check |
| __buffer_into_raw | move Buffer bytes to a raw address |
| __raw_free | free an address from __buffer_into_raw |
| __atom_from_bytes | Intern a static name from bytes and return its atom id. |
| __atom_from_string | Intern the contents of a String slot as an atom id. |
| __atom_name | Return the name of an atom id as a new String slot. |
| __atom_eq | Compare two atom ids; 1 if equal, else 0. |
| __label_new | Create a label slot from name bytes and one runtime payload. |
| __label_new_from_string | Create a label whose name comes from a String slot handle. |
| __label_name_eq | Compare a label's name to a static byte string. |
| __label_names_equal | Compare two label handles by name. |
| __label_payload | Return a cloned payload from a label; non-label → Unit. |
| __label_name | Return the label name as a new String slot. |
| __label_is_error | Return 1 if the value is a Label named `"error"`, else 0. |
| __error_label_from_str | Create `{:error, msg}` with a String payload from UTF-8 bytes. |
| __error_message_from_label | Return the error reason of an error label as a String slot. |
| __value_to_string | Convert a runtime value to a String slot for label interpolation. |
| __string_new | Allocate a String slot from a byte pointer and length. |
| __string_from_cstr | Allocate a String slot from a C string pointer. |
| __string_concat | Concatenate two String slots into a fresh String slot. |
| __string_eq | Compare two String slot handles by content. |
| __struct_new | Allocate a compatibility struct slab holding `size` bytes. Ordinary evaluated struct values still use this path. `StorageRep(struct)` itself is inline field layout, not this slab. |
| __struct_borrow | Borrow the raw struct pointer for field access. |
| __struct_track_value | Register a field value so struct drop/clone owns it. |
| __struct_forget_owned | Clear owned-field tracking after those fields have been moved into inline `StorageRep`. Does not drop payloads. The slab can then be `__drop`ped. |
| __sprs_set_output | Register a host output callback for `__println`. If none is registered, `__println` uses `eprintln!`. The compiler's `get_runtime_fn` does not declare this symbol. |

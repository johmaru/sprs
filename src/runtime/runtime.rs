//! Sprs runtime — slab (handle table) based memory management.
//!
//! All heap values (List/Range/String/Struct) are stored in a global slot
//! pool and addressed by a 64-bit *handle* packed as `(index:u32, generation:u32)`.
//! The C ABI never exchanges raw pointers across the boundary, which eliminates
//! NULL dereferences, dangling pointers (Vec reallocation), and use-after-free
//! (generation mismatch detection).
//!
//! Design notes:
//! - `thread_local! { RefCell<Vec<Slot>> }` keeps borrows inside each C ABI
//!   function. No reference escapes the closure, so no lifetime error.
//! - `SlotData` is a plain safe `enum` (no `ManuallyDrop`, no `union`); `Drop`
//!   is auto-implemented and frees the inner Vec/String/Box.
//! - `__list_get` takes the element and leaves `Unit` in that slot. OOB / bad
//!   handle is signalled by the `Unit` sentinel. List drop recursively
//!   `__drop`s remaining owned elements.
//! - Output (`__println`) is routed through a user-registerable callback
//!   (`__sprs_set_output`) so the runtime is host/stdout-agnostic. When no
//!   callback is registered, output falls back to `eprintln!` for host builds.
//!
//! Future `no_std` migration: replace `thread_local!` with
//! `critical_section::Mutex<Vec<Slot>>`; `RefCell` is already in `core`, and
//! the function bodies are unchanged.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Public value types (C ABI)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SprsValue {
    pub tag: i32,
    pub data: u64,
}

pub enum Tag {
    // Dynamic value tags
    Integer = 0, // i64
    Float = 1,   // f64
    String = 2,
    Boolean = 3,
    List = 4,
    Range = 5,
    Unit = 6,
    Struct = 8,
    Atom = 9, // immediate: data = interned atom id (u32 as u64). NOT a slab handle
    Label = 10,
    Buffer = 11,
    RawPtr = 12, // bare address in `data`; not a slab handle

    // System types
    Int8 = 100,
    Uint8 = 101,
    Int16 = 102,
    Uint16 = 103,
    Int32 = 104,
    Uint32 = 105,
    Int64 = 106,
    Uint64 = 107,

    Float16 = 108,
    Float32 = 109,
    Float64 = 110,
}

/// Return true for tags whose `data` field is a slab handle (needs `__drop`).
/// RawPtr is intentionally omitted: `data` is a bare address, not a slab handle.
/// Atom is omitted too: `data` is an interned id, an immediate value.
const fn is_heap_tag(tag: i32) -> bool {
    matches!(
        tag,
        tag_value if tag_value == Tag::String as i32
            || tag_value == Tag::List as i32
            || tag_value == Tag::Range as i32
            || tag_value == Tag::Struct as i32
            || tag_value == Tag::Label as i32
            || tag_value == Tag::Buffer as i32
    )
}

// ---------------------------------------------------------------------------
// Atom interning (process-global, never freed — Elixir-style)
// ---------------------------------------------------------------------------

struct AtomTable {
    to_id: HashMap<String, u32>,
    to_name: Vec<String>,
}

static ATOM_TABLE: OnceLock<Mutex<AtomTable>> = OnceLock::new();

fn atom_table() -> &'static Mutex<AtomTable> {
    ATOM_TABLE.get_or_init(|| {
        Mutex::new(AtomTable {
            to_id: HashMap::new(),
            to_name: Vec::new(),
        })
    })
}

/// Intern a name, returning its stable id. The same name always yields the
/// same id; ids are never reused or freed.
fn intern_atom(name: &str) -> u32 {
    let mut table = atom_table().lock().unwrap();
    if let Some(&id) = table.to_id.get(name) {
        return id;
    }
    let id = table.to_name.len() as u32;
    table.to_name.push(name.to_string());
    table.to_id.insert(name.to_string(), id);
    id
}

/// Reverse lookup: id → name. `None` for ids never handed out.
fn atom_name(id: u32) -> Option<String> {
    let table = atom_table().lock().unwrap();
    table.to_name.get(id as usize).cloned()
}

// ---------------------------------------------------------------------------
// Runtime value payloads (internal, never cross the C ABI)
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct SprsRange {
    pub start: i64,
    pub end: i64,
}

/// Safe enum: the variant acts as the tag, `Drop` frees the inner allocation
/// automatically. No `ManuallyDrop`, no `unsafe`.
enum SlotData {
    List(Vec<SprsValue>),
    Range(SprsRange),
    String(String),
    /// Raw struct bytes + the LLVM struct type id (for field access).
    /// Field access is performed by the codegen side via `build_struct_gep`,
    /// so the runtime treats struct payloads as opaque owned bytes.
    Struct {
        ptr: *mut u8,
        layout: std::alloc::Layout,
        /// Whether the runtime owns this allocation (false for empty/dangling
        /// sentinels so `Drop` won't dealloc a non-allocated pointer).
        owned: bool,
        owned_values: Vec<StructOwnedValue>,
    },
    Label {
        name: String,
        payload: SprsValue,
    },
    /// Fixed-size zero-initialized byte array (`new(n)`). `Vec` drops itself.
    Buffer(Vec<u8>),
    Empty,
}

#[derive(Clone)]
struct StructOwnedValue {
    offset: usize,
    value: SprsValue,
    data_only: bool,
}

impl Drop for SlotData {
    fn drop(&mut self) {
        match self {
            SlotData::List(values) => {
                let values = std::mem::take(values);
                for value in values {
                    __drop(value.tag, value.data);
                }
            }
            // String / SprsRange / Buffer drop themselves.
            SlotData::String(_) | SlotData::Range(_) | SlotData::Buffer(_) => {}
            SlotData::Label { payload, .. } => __drop(payload.tag, payload.data),
            SlotData::Struct {
                ptr,
                layout,
                owned,
                owned_values,
            } => {
                let tracked = std::mem::take(owned_values);
                for tracked_value in tracked {
                    __drop(tracked_value.value.tag, tracked_value.value.data);
                }
                if *owned && !ptr.is_null() {
                    unsafe { std::alloc::dealloc(*ptr as *mut u8, *layout) };
                }
            }
            SlotData::Empty => {}
        }
    }
}

struct Slot {
    generation: u32,
    data: SlotData,
}

// ---------------------------------------------------------------------------
// Global slot pool (thread_local + RefCell, borrows stay inside closures)
// ---------------------------------------------------------------------------

thread_local! {
    static SLOTS: RefCell<Vec<Slot>> = RefCell::new(Vec::new());
    static FREE_LIST: RefCell<Vec<u32>> = RefCell::new(Vec::new());
    static OUTPUT_FN: RefCell<Option<unsafe extern "C" fn(*const u8, usize)>> =
        RefCell::new(None);
    /// Address → Layout for allocations taken by `__buffer_into_raw`.
    /// Not generation tracking; RawPtr is a bare address.
    static RAW_LAYOUTS: RefCell<HashMap<usize, std::alloc::Layout>> =
        RefCell::new(HashMap::new());
}

// ---------------------------------------------------------------------------
// Handle encoding: (index:u32 | generation:u32) packed into u64.
// Handle value 0 is reserved as the invalid handle (index 0 is never used).
// ---------------------------------------------------------------------------

const INVALID_HANDLE: u64 = 0;

#[inline]
const fn handle_pack(index: u32, generation: u32) -> u64 {
    ((index as u64) << 32) | (generation as u64)
}

#[inline]
const fn handle_index(handle_value: u64) -> u32 {
    (handle_value >> 32) as u32
}

#[inline]
const fn handle_gen(handle_value: u64) -> u32 {
    handle_value as u32
}

/// Allocate a slot, returning a fresh handle. Index 0 is reserved.
fn slot_insert(data: SlotData) -> u64 {
    FREE_LIST.with(|fl| {
        SLOTS.with(|slots_cell| {
            let mut free_list = fl.borrow_mut();
            let mut slots = slots_cell.borrow_mut();
            if let Some(idx) = free_list.pop() {
                let slot = &mut slots[idx as usize];
                slot.generation = slot.generation.wrapping_add(1);
                if slot.generation == 0 {
                    slot.generation = 1; // skip 0 so the handle is never INVALID_HANDLE
                }
                slot.data = data;
                handle_pack(idx, slot.generation)
            } else {
                let idx = slots.len() as u32;
                if idx == 0 {
                    // Reserve index 0.
                    slots.push(Slot {
                        generation: 1,
                        data: SlotData::Empty,
                    });
                }
                let real_idx = slots.len() as u32;
                slots.push(Slot {
                    generation: 1,
                    data,
                });
                handle_pack(real_idx, 1)
            }
        })
    })
}

/// Release a slot. Safe to call with a stale handle (no-op on gen mismatch).
fn slot_release(handle: u64) {
    if handle == INVALID_HANDLE {
        return;
    }

    let released = SLOTS
        .try_with(|slots_cell| {
            let Ok(mut slots) = slots_cell.try_borrow_mut() else {
                return None;
            };
            let idx = handle_index(handle) as usize;
            if idx >= slots.len() {
                return None;
            }
            let slot = &mut slots[idx];
            if slot.generation != handle_gen(handle) {
                return None; // stale handle, not a live slot
            }
            // Move the payload out while SLOTS is borrowed, then drop it after
            // the borrow ends so nested heap payloads may release their slots.
            let data = std::mem::replace(&mut slot.data, SlotData::Empty);
            slot.generation = slot.generation.wrapping_add(1);
            if slot.generation == 0 {
                slot.generation = 1;
            }
            Some((idx as u32, data))
        })
        .ok()
        .flatten();

    if let Some((idx, data)) = released {
        drop(data);
        let _ = FREE_LIST.try_with(|fl| {
            if let Ok(mut list) = fl.try_borrow_mut() {
                list.push(idx);
            }
        });
    }
}

/// Read-only lookup that runs `f` with a reference to the payload if the
/// handle is live. Returns `f`'s result, or `default` on stale/invalid handle.
fn slot_with<ResultType, Callback>(
    handle: u64,
    default: ResultType,
    callback_function: Callback,
) -> ResultType
where
    Callback: FnOnce(&SlotData) -> ResultType,
{
    if handle == INVALID_HANDLE {
        return default;
    }
    SLOTS.with(|slots_cell| {
        let slots = slots_cell.borrow();
        let idx = handle_index(handle) as usize;
        let Some(slot) = slots.get(idx) else {
            return default;
        };
        if slot.generation != handle_gen(handle) {
            return default;
        }
        callback_function(&slot.data)
    })
}

// ---------------------------------------------------------------------------
// Output abstraction (user-registerable callback)
// ---------------------------------------------------------------------------

/// Register a function that receives raw bytes for output. The runtime uses
/// this for `__println`. If never called, output falls back to `eprintln!`.
#[unsafe(no_mangle)]
pub extern "C" fn __sprs_set_output(callback_function: unsafe extern "C" fn(*const u8, usize)) {
    OUTPUT_FN.with(|cell| *cell.borrow_mut() = Some(callback_function));
}

fn sprs_out(bytes: &[u8]) {
    OUTPUT_FN.with(|cell| {
        let opt = *cell.borrow();
        if let Some(callback_function) = opt {
            // SAFETY: caller of `__sprs_set_output` guarantees the function
            // handles the pointer/length pair correctly.
            unsafe { callback_function(bytes.as_ptr(), bytes.len()) };
        } else {
            // Fallback for host builds: stderr.
            use std::io::Write;
            let _ = std::io::stderr().write_all(bytes);
        }
    });
}

fn sprs_out_str(output_text: &str) {
    sprs_out(output_text.as_bytes());
}

fn sprs_out_line(output_text: &str) {
    sprs_out_str(output_text);
    sprs_out(b"\n");
}

// ---------------------------------------------------------------------------
// Float16 conversion (kept from the previous implementation)
// ---------------------------------------------------------------------------

fn f16_tof32(bit: u16) -> f32 {
    let sign = (bit >> 15) as u32;
    let exp = ((bit >> 10) & 0x1F) as u32;
    let mant = (bit & 0x3FF) as u32;

    if exp == 0 {
        if mant == 0 {
            f32::from_bits(sign << 31)
        } else {
            let val = mant as f32 / 16777216.0; // 2^24
            if sign == 1 { -val } else { val }
        }
    } else if exp == 31 {
        if mant == 0 {
            f32::from_bits((sign << 31) | 0x7F800000)
        } else {
            f32::from_bits((sign << 31) | 0x7F800000 | (mant << 13))
        }
    } else {
        let new_exp = exp + 112;
        f32::from_bits((sign << 31) | (new_exp << 23) | (mant << 13))
    }
}

// ---------------------------------------------------------------------------
// C ABI: List
// ---------------------------------------------------------------------------

/// Maximum list capacity to avoid OOM on absurd input.
const MAX_LIST_CAPACITY: i64 = 1 << 30;

#[unsafe(no_mangle)]
pub extern "C" fn __list_new(capacity: i64) -> u64 {
    if capacity < 0 || capacity > MAX_LIST_CAPACITY {
        return INVALID_HANDLE;
    }
    let cap = capacity as usize;
    let vec = if cap == 0 {
        Vec::new()
    } else {
        Vec::with_capacity(cap)
    };
    slot_insert(SlotData::List(vec))
}

#[unsafe(no_mangle)]
pub extern "C" fn __list_push(list_handle: u64, tag: i32, data: u64) {
    SLOTS.with(|slots_cell| {
        let mut slots = slots_cell.borrow_mut();
        let idx = handle_index(list_handle) as usize;
        if idx >= slots.len() || list_handle == INVALID_HANDLE {
            return;
        }
        let slot = &mut slots[idx];
        if slot.generation != handle_gen(list_handle) {
            return;
        }
        if let SlotData::List(vec) = &mut slot.data {
            vec.push(SprsValue { tag, data });
        }
    });
}

/// Returns a bare `SprsValue` by value. On bad handle / wrong type / OOB,
/// returns the `Unit` sentinel `{ tag: Unit, data: 0 }`.
#[unsafe(no_mangle)]
pub extern "C" fn __list_get(list_handle: u64, index: i64) -> SprsValue {
    let sentinel = SprsValue {
        tag: Tag::Unit as i32,
        data: 0,
    };
    if list_handle == INVALID_HANDLE {
        return sentinel;
    }
    SLOTS.with(|slots_cell| {
        let mut slots = slots_cell.borrow_mut();
        let idx = handle_index(list_handle) as usize;
        if idx >= slots.len() {
            return sentinel;
        }
        let slot = &mut slots[idx];
        if slot.generation != handle_gen(list_handle) {
            return sentinel;
        }
        let SlotData::List(vec) = &mut slot.data else {
            return sentinel;
        };
        if index < 0 || (index as usize) >= vec.len() {
            return sentinel;
        }
        std::mem::replace(&mut vec[index as usize], sentinel)
    })
}

/// Replace the element at `index`. Drops the previous value. OOB / bad handle
/// is a no-op (the incoming value is dropped so it is not leaked).
#[unsafe(no_mangle)]
pub extern "C" fn __list_set(list_handle: u64, index: i64, tag: i32, data: u64) {
    let incoming = SprsValue { tag, data };
    let mut prev: Option<SprsValue> = None;
    let stored = SLOTS.with(|slots_cell| {
        let mut slots = slots_cell.borrow_mut();
        let idx = handle_index(list_handle) as usize;
        if idx >= slots.len() || list_handle == INVALID_HANDLE {
            return false;
        }
        let slot = &mut slots[idx];
        if slot.generation != handle_gen(list_handle) {
            return false;
        }
        let SlotData::List(vec) = &mut slot.data else {
            return false;
        };
        if index < 0 || (index as usize) >= vec.len() {
            return false;
        }
        prev = Some(std::mem::replace(&mut vec[index as usize], incoming));
        true
    });
    if stored {
        if let Some(prev) = prev {
            __drop(prev.tag, prev.data);
        }
    } else {
        __drop(incoming.tag, incoming.data);
    }
}

// ---------------------------------------------------------------------------
// C ABI: Buffer
// ---------------------------------------------------------------------------

/// Allocate a zero-initialized Buffer of `size` bytes. Negative size →
/// INVALID_HANDLE. Size 0 → a valid handle to an empty buffer.
#[unsafe(no_mangle)]
pub extern "C" fn __buffer_new(size: i64) -> u64 {
    if size < 0 {
        return INVALID_HANDLE;
    }
    let bytes = if size == 0 {
        Vec::new()
    } else {
        vec![0u8; size as usize]
    };
    slot_insert(SlotData::Buffer(bytes))
}

/// Length of a live Buffer as i64; stale / non-Buffer handle → 0.
#[unsafe(no_mangle)]
pub extern "C" fn __buffer_len(handle: u64) -> i64 {
    slot_with(handle, 0, |slot_data| match slot_data {
        SlotData::Buffer(bytes) => bytes.len() as i64,
        _ => 0,
    })
}

/// Read one byte as an Integer value. OOB / stale / non-Buffer → Unit sentinel.
#[unsafe(no_mangle)]
pub extern "C" fn __buffer_get(handle: u64, index: i64) -> SprsValue {
    let sentinel = SprsValue {
        tag: Tag::Unit as i32,
        data: 0,
    };
    slot_with(handle, sentinel, |slot_data| match slot_data {
        SlotData::Buffer(bytes) => {
            if index < 0 || (index as usize) >= bytes.len() {
                sentinel
            } else {
                SprsValue {
                    tag: Tag::Integer as i32,
                    data: bytes[index as usize] as u64,
                }
            }
        }
        _ => sentinel,
    })
}

/// Write one byte (low 8 bits of `value`). OOB / stale / non-Buffer → no-op.
#[unsafe(no_mangle)]
pub extern "C" fn __buffer_set(handle: u64, index: i64, value: i64) {
    SLOTS.with(|slots_cell| {
        let mut slots = slots_cell.borrow_mut();
        let idx = handle_index(handle) as usize;
        if idx >= slots.len() || handle == INVALID_HANDLE {
            return;
        }
        let slot = &mut slots[idx];
        if slot.generation != handle_gen(handle) {
            return;
        }
        if let SlotData::Buffer(bytes) = &mut slot.data {
            if index >= 0 && (index as usize) < bytes.len() {
                bytes[index as usize] = value as u8;
            }
        }
    });
}

/// Returns 1 if `handle` refers to a live Buffer slot, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn __buffer_exist(handle: u64) -> i32 {
    slot_with(handle, 0, |slot_data| match slot_data {
        SlotData::Buffer(_) => 1,
        _ => 0,
    })
}

/// Take ownership of a live Buffer's byte allocation and return it as a raw
/// address (the `data` of a `Tag::RawPtr` value). The slot is emptied and
/// released, so the Buffer handle becomes stale; the caller owns the memory
/// and must release it with `__raw_free`.
///
/// Returns 0 for stale / non-Buffer handles and for empty (len 0) buffers.
#[unsafe(no_mangle)]
pub extern "C" fn __buffer_into_raw(handle: u64) -> u64 {
    if handle == INVALID_HANDLE {
        return 0;
    }
    let taken = SLOTS.with(|slots_cell| {
        let mut slots = slots_cell.borrow_mut();
        let idx = handle_index(handle) as usize;
        if idx >= slots.len() {
            return None;
        }
        let slot = &mut slots[idx];
        if slot.generation != handle_gen(handle) {
            return None;
        }
        let SlotData::Buffer(bytes) = &mut slot.data else {
            return None;
        };
        if bytes.is_empty() {
            return None;
        }
        // Forget the Vec so Drop won't free it; caller owns the allocation via RAW_LAYOUTS.
        let mut vec = std::mem::replace(bytes, Vec::new());
        let addr = vec.as_mut_ptr() as usize;
        let cap = vec.capacity();
        std::mem::forget(vec);
        slot.data = SlotData::Empty;
        slot.generation = slot.generation.wrapping_add(1);
        if slot.generation == 0 {
            slot.generation = 1;
        }
        Some((idx as u32, addr, cap))
    });
    match taken {
        Some((idx, addr, cap)) => {
            let layout = std::alloc::Layout::from_size_align(cap, 1)
                .unwrap_or_else(|_| std::alloc::Layout::from_size_align(cap.max(1), 1).unwrap());
            RAW_LAYOUTS.with(|layouts| {
                layouts.borrow_mut().insert(addr, layout);
            });
            FREE_LIST.with(|fl| fl.borrow_mut().push(idx));
            addr as u64
        }
        None => 0,
    }
}

/// Release a raw pointer previously returned by `__buffer_into_raw`.
/// Null / unknown pointers are silently ignored (C-equivalent double-free
/// policy; no panic). Freeing a pointer this runtime did not allocate is a
/// user error.
#[unsafe(no_mangle)]
pub extern "C" fn __raw_free(ptr: u64) {
    if ptr == 0 {
        return;
    }
    let addr = ptr as usize;
    let layout = RAW_LAYOUTS.with(|layouts| layouts.borrow_mut().remove(&addr));
    if let Some(layout) = layout {
        unsafe { std::alloc::dealloc(addr as *mut u8, layout) };
    }
}

// ---------------------------------------------------------------------------
// C ABI: Range
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __range_new(start: i64, end: i64) -> u64 {
    slot_insert(SlotData::Range(SprsRange { start, end }))
}

// ---------------------------------------------------------------------------
// C ABI: String (slot-backed)
//
// Codegen allocates a String slot by calling `__string_new(bytes_ptr, len)`.
// `__string_from_cstr` is provided for the legacy path where the caller has a
// NUL-terminated C string (e.g. an LLVM global).
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __string_new(bytes_ptr: *const u8, len: i64) -> u64 {
    if bytes_ptr.is_null() || len < 0 {
        return INVALID_HANDLE;
    }
    let bytes = unsafe { std::slice::from_raw_parts(bytes_ptr, len as usize) };
    let string_value = String::from_utf8_lossy(bytes).into_owned();
    slot_insert(SlotData::String(string_value))
}

#[unsafe(no_mangle)]
pub extern "C" fn __string_from_cstr(cstr_ptr: *const i8) -> u64 {
    if cstr_ptr.is_null() {
        return INVALID_HANDLE;
    }
    let c_str = unsafe { CStr::from_ptr(cstr_ptr) };
    let string_value = c_str.to_string_lossy().into_owned();
    slot_insert(SlotData::String(string_value))
}

/// Concatenate two String slots into a fresh String slot. Read-then-release-
/// then-insert pattern: clone both source strings out of their slots (dropping
/// the borrows), then allocate the concatenated string in a new slot. This
/// avoids the dangling-pointer issue that `__string_borrow` had and moves the
/// `l_len + r_len` overflow / memcpy logic into safe Rust, eliminating
/// BUG-L02 (string concat heap buffer overflow).
#[unsafe(no_mangle)]
pub extern "C" fn __string_concat(left_handle: u64, right_handle: u64) -> u64 {
    let left_text: Option<String> = slot_with(left_handle, None, |slot_data| match slot_data {
        SlotData::String(string_value) => Some(string_value.clone()),
        _ => None,
    });
    let right_text: Option<String> = slot_with(right_handle, None, |slot_data| match slot_data {
        SlotData::String(string_value) => Some(string_value.clone()),
        _ => None,
    });
    match (left_text, right_text) {
        (Some(mut left_text), Some(right_text)) => {
            left_text.push_str(&right_text);
            slot_insert(SlotData::String(left_text))
        }
        _ => INVALID_HANDLE,
    }
}

/// Compare two String slot handles by content. Returns 1 if both handles are
/// live `SlotData::String` values with equal text, otherwise 0 (including
/// stale / `INVALID_HANDLE` / non-String slots).
#[unsafe(no_mangle)]
pub extern "C" fn __string_eq(a: u64, b: u64) -> i32 {
    let left_text: Option<String> = slot_with(a, None, |slot_data| match slot_data {
        SlotData::String(string_value) => Some(string_value.clone()),
        _ => None,
    });
    let right_text: Option<String> = slot_with(b, None, |slot_data| match slot_data {
        SlotData::String(string_value) => Some(string_value.clone()),
        _ => None,
    });
    match (left_text, right_text) {
        (Some(left_text), Some(right_text)) if left_text == right_text => 1,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// C ABI: Struct
// ---------------------------------------------------------------------------

/// Allocate a struct slot holding `size` bytes (codegen fills them in via the
/// returned raw pointer). The slot owns the allocation and frees it on drop.
#[unsafe(no_mangle)]
pub extern "C" fn __struct_new(size: i64) -> u64 {
    if size <= 0 {
        // Distinguish "empty struct" from an allocation failure by returning
        // a valid handle with a dangling but non-null pointer.
        return slot_insert(SlotData::Struct {
            ptr: std::mem::align_of::<u8>() as *mut u8, // non-null sentinel
            layout: std::alloc::Layout::new::<u8>(),
            owned: false, // dangling sentinel, never dealloc'd
            owned_values: Vec::new(),
        });
    }
    let size_us = size as usize;
    let layout = match std::alloc::Layout::from_size_align(size_us, 8) {
        Ok(layout_value) => layout_value,
        Err(_) => return INVALID_HANDLE,
    };
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        return INVALID_HANDLE;
    }
    slot_insert(SlotData::Struct {
        ptr,
        layout,
        owned: true,
        owned_values: Vec::new(),
    })
}

/// Borrow the raw struct pointer for field access. Valid until the slot is
/// mutated or released.
#[unsafe(no_mangle)]
pub extern "C" fn __struct_borrow(handle: u64) -> *mut u8 {
    if handle == INVALID_HANDLE {
        return std::ptr::null_mut();
    }
    SLOTS.with(|slots_cell| {
        let slots = slots_cell.borrow();
        let idx = handle_index(handle) as usize;
        let Some(slot) = slots.get(idx) else {
            return std::ptr::null_mut();
        };
        if slot.generation != handle_gen(handle) {
            return std::ptr::null_mut();
        }
        match &slot.data {
            SlotData::Struct { ptr, .. } => *ptr,
            _ => std::ptr::null_mut::<u8>(),
        }
    })
}

/// Register a field value so struct drop/clone owns it independently of
/// the original variable binding.
#[unsafe(no_mangle)]
pub extern "C" fn __struct_track_value(
    handle: u64,
    field_ptr: *mut u8,
    tag: i32,
    data: u64,
    data_only: i32,
) -> i32 {
    if handle == INVALID_HANDLE || field_ptr.is_null() {
        return 0;
    }
    let nbytes = if data_only != 0 {
        8usize
    } else {
        std::mem::size_of::<SprsValue>()
    };
    let field_addr = field_ptr as usize;
    let outcome: Option<Option<SprsValue>> = SLOTS.with(|slots_cell| {
        let mut slots = slots_cell.borrow_mut();
        let idx = handle_index(handle) as usize;
        let Some(slot) = slots.get_mut(idx) else {
            return None;
        };
        if slot.generation != handle_gen(handle) {
            return None;
        }
        let SlotData::Struct {
            ptr,
            layout,
            owned_values,
            ..
        } = &mut slot.data
        else {
            return None;
        };
        let base = *ptr as usize;
        if field_addr < base {
            return None;
        }
        let offset = field_addr - base;
        if offset
            .checked_add(nbytes)
            .map(|end| end > layout.size())
            .unwrap_or(true)
        {
            return None;
        }
        let incoming = StructOwnedValue {
            offset,
            value: SprsValue { tag, data },
            data_only: data_only != 0,
        };
        let previous = if let Some(pos) = owned_values.iter().position(|v| v.offset == offset) {
            let old = owned_values[pos].value;
            owned_values[pos] = incoming;
            Some(old)
        } else {
            owned_values.push(incoming);
            None
        };
        Some(previous)
    });
    match outcome {
        None => 0,
        Some(previous) => {
            if let Some(old) = previous {
                __drop(old.tag, old.data);
            }
            1
        }
    }
}

/// Drop tracking for moved-out struct fields without dropping payloads.
/// The struct allocation can then be released with `__drop`.
#[unsafe(no_mangle)]
pub extern "C" fn __struct_forget_owned(handle: u64) -> i32 {
    if handle == INVALID_HANDLE {
        return 0;
    }
    SLOTS.with(|slots_cell| {
        let mut slots = slots_cell.borrow_mut();
        let idx = handle_index(handle) as usize;
        let Some(slot) = slots.get_mut(idx) else {
            return 0;
        };
        if slot.generation != handle_gen(handle) {
            return 0;
        }
        let SlotData::Struct { owned_values, .. } = &mut slot.data else {
            return 0;
        };
        owned_values.clear();
        1
    })
}

/// Create a label slot containing a copied name and one runtime payload.
#[unsafe(no_mangle)]
pub extern "C" fn __label_new(
    name_ptr: *const u8,
    name_len: i64,
    payload_tag: i32,
    payload_data: u64,
) -> u64 {
    if name_ptr.is_null() || name_len < 0 {
        return INVALID_HANDLE;
    }
    let bytes = unsafe { std::slice::from_raw_parts(name_ptr, name_len as usize) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    slot_insert(SlotData::Label {
        name,
        payload: SprsValue {
            tag: payload_tag,
            data: payload_data,
        },
    })
}

/// Convert a runtime value to a String slot for dynamic label interpolation.
/// Supported: String (clone), Integer (decimal), Boolean ("true"/"false").
/// Other tags return INVALID_HANDLE.
#[unsafe(no_mangle)]
pub extern "C" fn __value_to_string(tag: i32, data: u64) -> u64 {
    if tag == Tag::String as i32 {
        return string_clone(data);
    }
    if tag == Tag::Integer as i32 {
        let formatted_text = (data as i64).to_string();
        return slot_insert(SlotData::String(formatted_text));
    }
    if tag == Tag::Boolean as i32 {
        let formatted_text = if data != 0 { "true" } else { "false" };
        return slot_insert(SlotData::String(formatted_text.to_string()));
    }
    INVALID_HANDLE
}

/// Create a label whose name comes from an existing String slot handle.
/// The name string is cloned; `name_handle` itself is not consumed.
#[unsafe(no_mangle)]
pub extern "C" fn __label_new_from_string(
    name_handle: u64,
    payload_tag: i32,
    payload_data: u64,
) -> u64 {
    let name: Option<String> = slot_with(name_handle, None, |slot_data| match slot_data {
        SlotData::String(string_value) => Some(string_value.clone()),
        _ => None,
    });
    let Some(name) = name else {
        return INVALID_HANDLE;
    };
    slot_insert(SlotData::Label {
        name,
        payload: SprsValue {
            tag: payload_tag,
            data: payload_data,
        },
    })
}

/// Compare a label's name to a static byte string. Returns 1 on match, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn __label_name_eq(handle: u64, name_ptr: *const u8, name_len: i64) -> i32 {
    if name_ptr.is_null() || name_len < 0 {
        return 0;
    }
    let expected = unsafe { std::slice::from_raw_parts(name_ptr, name_len as usize) };
    slot_with(handle, 0i32, |slot_data| match slot_data {
        SlotData::Label { name, .. } => {
            if name.as_bytes() == expected {
                1
            } else {
                0
            }
        }
        _ => 0,
    })
}

/// Compare two label handles by name. Returns 1 on match, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn __label_names_equal(first_value: u64, second_value: u64) -> i32 {
    let name_a: Option<String> = slot_with(first_value, None, |slot_data| match slot_data {
        SlotData::Label { name, .. } => Some(name.clone()),
        _ => None,
    });
    let name_b: Option<String> = slot_with(second_value, None, |slot_data| match slot_data {
        SlotData::Label { name, .. } => Some(name.clone()),
        _ => None,
    });
    match (name_a, name_b) {
        (Some(first_value), Some(second_value)) if first_value == second_value => 1,
        _ => 0,
    }
}

/// Intern a static name (bytes) and return its atom id.
#[unsafe(no_mangle)]
pub extern "C" fn __atom_from_bytes(name_ptr: *const u8, name_len: i64) -> u64 {
    if name_ptr.is_null() || name_len < 0 {
        return u64::MAX;
    }
    let bytes = unsafe { std::slice::from_raw_parts(name_ptr, name_len as usize) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    u64::from(intern_atom(&name))
}

/// Intern the contents of a String slot as an atom id.
#[unsafe(no_mangle)]
pub extern "C" fn __atom_from_string(name_handle: u64) -> u64 {
    let name: Option<String> = slot_with(name_handle, None, |slot_data| match slot_data {
        SlotData::String(string_value) => Some(string_value.clone()),
        _ => None,
    });
    match name {
        Some(string_value) => u64::from(intern_atom(&string_value)),
        None => u64::MAX,
    }
}

/// Return the name of an atom id as a new String slot ("" for invalid ids).
#[unsafe(no_mangle)]
pub extern "C" fn __atom_name(id: u64) -> u64 {
    let name = atom_name(id as u32).unwrap_or_default();
    slot_insert(SlotData::String(name))
}

/// Compare two atom ids. Returns 1 on equality, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn __atom_eq(left: u64, right: u64) -> i32 {
    i32::from(left == right)
}

/// Return a cloned payload from a label. Non-label → Unit.
#[unsafe(no_mangle)]
pub extern "C" fn __label_payload(handle: u64) -> SprsValue {
    let payload: Option<SprsValue> = slot_with(handle, None, |slot_data| match slot_data {
        SlotData::Label { payload, .. } => Some(*payload),
        _ => None,
    });
    match payload {
        Some(payload_value) => __clone(payload_value.tag, payload_value.data),
        None => SprsValue {
            tag: Tag::Unit as i32,
            data: 0,
        },
    }
}

/// Return the label name as a new String slot. Non-label → empty string slot.
#[unsafe(no_mangle)]
pub extern "C" fn __label_name(handle: u64) -> u64 {
    let name: Option<String> = slot_with(handle, None, |slot_data| match slot_data {
        SlotData::Label { name, .. } => Some(name.clone()),
        _ => None,
    });
    match name {
        Some(string_value) => slot_insert(SlotData::String(string_value)),
        None => slot_insert(SlotData::String(String::new())),
    }
}

// ---------------------------------------------------------------------------
// C ABI: Drop
//
// `__drop(tag, data)` — `data` is a slab handle for heap tags, or an immediate
// value for primitive tags. Primitive tags are a no-op.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __drop(tag: i32, data: u64) {
    if !is_heap_tag(tag) {
        return;
    }
    if SLOTS.try_with(|_| {}).is_err() {
        return;
    }
    slot_release(data);
}

// ---------------------------------------------------------------------------
// C ABI: Clone (deep copy)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __clone(tag: i32, data: u64) -> SprsValue {
    // Primitive / immediate tags: copy as-is.
    if !is_heap_tag(tag) {
        return SprsValue { tag, data };
    }

    if tag == Tag::String as i32 {
        let new_handle = string_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }

    if tag == Tag::List as i32 {
        let new_handle = list_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }

    if tag == Tag::Range as i32 {
        let new_handle = range_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }

    if tag == Tag::Struct as i32 {
        let new_handle = struct_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }

    if tag == Tag::Label as i32 {
        let new_handle = label_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }

    if tag == Tag::Buffer as i32 {
        let new_handle = buffer_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }

    // Unknown heap tag: return Unit.
    SprsValue {
        tag: Tag::Unit as i32,
        data: 0,
    }
}

/// Read the String out of the slot (clone the bytes), release the borrow,
/// then insert a fresh slot. Calling `slot_insert` inside `slot_with` would
/// re-borrow SLOTS mutably while a shared borrow is live → runtime panic.
fn string_clone(handle: u64) -> u64 {
    let cloned: Option<String> = slot_with(handle, None, |slot_data| match slot_data {
        SlotData::String(string_value) => Some(string_value.clone()),
        _ => None,
    });
    match cloned {
        Some(string_value) => slot_insert(SlotData::String(string_value)),
        None => INVALID_HANDLE,
    }
}

/// Same read-then-release-then-insert pattern as `string_clone`.
fn range_clone(handle: u64) -> u64 {
    let cloned: Option<SprsRange> = slot_with(handle, None, |slot_data| match slot_data {
        SlotData::Range(range_value) => Some(SprsRange {
            start: range_value.start,
            end: range_value.end,
        }),
        _ => None,
    });
    match cloned {
        Some(range_value) => slot_insert(SlotData::Range(range_value)),
        None => INVALID_HANDLE,
    }
}

fn list_clone(handle: u64) -> u64 {
    // Collect the source elements by cloning each one, then insert a new list
    // slot holding the cloned vector. Done in two passes because `slot_insert`
    // borrows SLOTS mutably and we can't hold a borrowed reference across it.
    let snapshot: Vec<SprsValue> = slot_with(handle, Vec::new(), |slot_data| match slot_data {
        SlotData::List(list_values) => list_values.clone(),
        _ => Vec::new(),
    });
    if snapshot.is_empty() && !slot_is_list(handle) {
        return INVALID_HANDLE;
    }
    let mut new_vec = Vec::with_capacity(snapshot.len());
    for val in snapshot {
        new_vec.push(__clone(val.tag, val.data));
    }
    slot_insert(SlotData::List(new_vec))
}

fn slot_is_list(handle: u64) -> bool {
    slot_with(handle, false, |slot_data| {
        matches!(slot_data, SlotData::List(_))
    })
}

fn struct_clone(handle: u64) -> u64 {
    let snapshot: Option<(*mut u8, std::alloc::Layout, usize, Vec<StructOwnedValue>)> =
        slot_with(handle, None, |slot_data| match slot_data {
            SlotData::Struct {
                ptr,
                layout,
                owned_values,
                ..
            } => Some((*ptr, *layout, layout.size(), owned_values.clone())),
            _ => None,
        });
    let Some((ptr, layout, size, owned_values)) = snapshot else {
        return INVALID_HANDLE;
    };
    if ptr.is_null() || size == 0 {
        return slot_insert(SlotData::Struct {
            ptr: std::mem::align_of::<u8>() as *mut u8,
            layout: std::alloc::Layout::new::<u8>(),
            owned: false,
            owned_values: Vec::new(),
        });
    }
    let new_ptr = unsafe { std::alloc::alloc(layout) };
    if new_ptr.is_null() {
        return INVALID_HANDLE;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(ptr, new_ptr, size);
    }

    let mut new_owned = Vec::with_capacity(owned_values.len());
    let mut cloned_so_far: Vec<SprsValue> = Vec::new();
    for tracked in owned_values {
        let cloned = __clone(tracked.value.tag, tracked.value.data);
        if is_heap_tag(tracked.value.tag) && cloned.data == INVALID_HANDLE {
            for previous in cloned_so_far {
                __drop(previous.tag, previous.data);
            }
            unsafe {
                std::alloc::dealloc(new_ptr, layout);
            }
            return INVALID_HANDLE;
        }
        cloned_so_far.push(cloned);
        unsafe {
            if tracked.data_only {
                std::ptr::write(new_ptr.add(tracked.offset) as *mut u64, cloned.data);
            } else {
                std::ptr::write(new_ptr.add(tracked.offset) as *mut SprsValue, cloned);
            }
        }
        new_owned.push(StructOwnedValue {
            offset: tracked.offset,
            value: cloned,
            data_only: tracked.data_only,
        });
    }

    slot_insert(SlotData::Struct {
        ptr: new_ptr,
        layout,
        owned: true,
        owned_values: new_owned,
    })
}

fn label_clone(handle: u64) -> u64 {
    let snapshot: Option<(String, SprsValue)> =
        slot_with(handle, None, |slot_data| match slot_data {
            SlotData::Label { name, payload } => Some((name.clone(), *payload)),
            _ => None,
        });
    let Some((name, payload)) = snapshot else {
        return INVALID_HANDLE;
    };
    slot_insert(SlotData::Label {
        name,
        payload: __clone(payload.tag, payload.data),
    })
}

/// Same read-then-release-then-insert pattern as the other clone helpers:
/// deep-copy the byte vector into a fresh slot.
fn buffer_clone(handle: u64) -> u64 {
    let cloned: Option<Vec<u8>> = slot_with(handle, None, |slot_data| match slot_data {
        SlotData::Buffer(bytes) => Some(bytes.clone()),
        _ => None,
    });
    match cloned {
        Some(bytes) => slot_insert(SlotData::Buffer(bytes)),
        None => INVALID_HANDLE,
    }
}

// ---------------------------------------------------------------------------
// C ABI: error labels (`{:error, …}`)
// ---------------------------------------------------------------------------

/// Returns 1 (true) if the value is a `Label` whose name is "error",
/// 0 (false) otherwise. Replaces the legacy `Tag::Error` check.
#[unsafe(no_mangle)]
pub extern "C" fn __label_is_error(tag: i32, data: u64) -> i32 {
    if tag != Tag::Label as i32 {
        return 0;
    }
    __label_name_eq(data, b"error".as_ptr(), 5)
}

/// Create an error label `{:error, msg}` whose payload is a fresh String slot
/// built from the given UTF-8 bytes. Used by arithmetic/cast/shift failure
/// paths so the reason is carried as a normal label payload.
#[unsafe(no_mangle)]
pub extern "C" fn __error_label_from_str(bytes_ptr: *const u8, len: i64) -> u64 {
    let msg_handle = __string_new(bytes_ptr, len);
    if msg_handle == INVALID_HANDLE {
        return INVALID_HANDLE;
    }
    __label_new(b"error".as_ptr(), 5, Tag::String as i32, msg_handle)
}

/// Return the error reason of an error label as a new String slot.
/// - Label named "error" with a String payload: clone of that payload.
/// - Label named "error" with any other payload: `format_sprs_value` rendering.
/// - Anything else (including non-labels): empty string slot (never INVALID).
#[unsafe(no_mangle)]
pub extern "C" fn __error_message_from_label(handle: u64) -> u64 {
    let (name, payload) = slot_with(
        handle,
        (
            String::new(),
            SprsValue {
                tag: Tag::Unit as i32,
                data: 0,
            },
        ),
        |slot_data| match slot_data {
            SlotData::Label { name, payload } => (name.clone(), *payload),
            _ => (
                String::new(),
                SprsValue {
                    tag: Tag::Unit as i32,
                    data: 0,
                },
            ),
        },
    );
    if name != "error" {
        return slot_insert(SlotData::String(String::new()));
    }
    if payload.tag == Tag::String as i32 {
        let cloned = string_clone(payload.data);
        if cloned != INVALID_HANDLE {
            return cloned;
        }
        return slot_insert(SlotData::String(String::new()));
    }
    let mut buf = String::new();
    format_sprs_value(&payload, &mut buf);
    slot_insert(SlotData::String(buf))
}

// ---------------------------------------------------------------------------
// C ABI: __strlen (legacy — reads a String slot's length)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __strlen(handle: u64) -> i64 {
    slot_with(handle, 0i64, |slot_data| match slot_data {
        SlotData::String(string_value) => string_value.len() as i64,
        _ => 0,
    })
}

// ---------------------------------------------------------------------------
// C ABI: __malloc (legacy, kept for compat)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __malloc(size: i64) -> *mut i8 {
    if size <= 0 {
        return std::ptr::null_mut();
    }
    let layout = match std::alloc::Layout::from_size_align(size as usize, 8) {
        Ok(layout_value) => layout_value,
        Err(_) => return std::ptr::null_mut(),
    };
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    ptr as *mut i8
}

// ---------------------------------------------------------------------------
// C ABI: __println (user-callback routed)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __println(list_handle: u64) {
    // Read the list elements out (clone the Vec), release the borrow, then
    // format each value. Cloning avoids holding a borrow while calling
    // `format_sprs_value`, which itself may call `slot_with`.
    let snapshot: Vec<SprsValue> =
        slot_with(list_handle, Vec::new(), |slot_data| match slot_data {
            SlotData::List(list_values) => list_values.clone(),
            _ => Vec::new(),
        });
    if snapshot.is_empty() && !slot_is_list(list_handle) {
        // Invalid handle: print nothing.
        return;
    }
    let mut buf = String::new();
    for (item_index, val) in snapshot.iter().enumerate() {
        if item_index > 0 {
            buf.push(' ');
        }
        format_sprs_value(val, &mut buf);
    }
    sprs_out_line(&buf);
}

fn format_sprs_value(val: &SprsValue, out: &mut String) {
    match val.tag {
        tag_value if tag_value == Tag::Integer as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i64);
        }
        tag_value if tag_value == Tag::Float as i32 || tag_value == Tag::Float64 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", f64::from_bits(val.data));
        }
        tag_value if tag_value == Tag::Float16 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", f16_tof32(val.data as u16));
        }
        tag_value if tag_value == Tag::Float32 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", f32::from_bits(val.data as u32));
        }
        tag_value if tag_value == Tag::String as i32 => {
            slot_with(val.data, (), |slot_data| match slot_data {
                SlotData::String(string_value) => out.push_str(string_value),
                _ => out.push_str("<invalid string>"),
            });
        }
        tag_value if tag_value == Tag::Boolean as i32 => {
            out.push_str(if val.data != 0 { "true" } else { "false" });
        }
        tag_value if tag_value == Tag::List as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "<list handle {:016x}>", val.data);
        }
        tag_value if tag_value == Tag::Range as i32 => {
            use std::fmt::Write;
            let (start, end) = slot_with(val.data, (0i64, 0i64), |slot_data| match slot_data {
                SlotData::Range(range_value) => (range_value.start, range_value.end),
                _ => (0, 0),
            });
            let _ = write!(out, "<range {}..{}>", start, end);
        }
        tag_value if tag_value == Tag::Int8 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i8);
        }
        tag_value if tag_value == Tag::Uint8 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u8);
        }
        tag_value if tag_value == Tag::Int16 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i16);
        }
        tag_value if tag_value == Tag::Uint16 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u16);
        }
        tag_value if tag_value == Tag::Int32 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i32);
        }
        tag_value if tag_value == Tag::Uint32 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u32);
        }
        tag_value if tag_value == Tag::Int64 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i64);
        }
        tag_value if tag_value == Tag::Uint64 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u64);
        }
        tag_value if tag_value == Tag::Unit as i32 => {
            out.push_str("()");
        }
        tag_value if tag_value == Tag::Struct as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "<struct handle {:016x}>", val.data);
        }
        tag_value if tag_value == Tag::Buffer as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "Buffer({})", __buffer_len(val.data));
        }
        tag_value if tag_value == Tag::RawPtr as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "RawPtr(0x{:x})", val.data);
        }
        tag_value if tag_value == Tag::Atom as i32 => {
            out.push(':');
            match atom_name(val.data as u32) {
                Some(name) => out.push_str(&name),
                None => out.push_str("<?>"),
            }
        }
        tag_value if tag_value == Tag::Label as i32 => {
            let (name, payload) = slot_with(
                val.data,
                (
                    String::new(),
                    SprsValue {
                        tag: Tag::Unit as i32,
                        data: 0,
                    },
                ),
                |slot_data| match slot_data {
                    SlotData::Label { name, payload } => (name.clone(), *payload),
                    _ => (
                        String::new(),
                        SprsValue {
                            tag: Tag::Unit as i32,
                            data: 0,
                        },
                    ),
                },
            );
            out.push_str("{:");
            out.push_str(&name);
            out.push_str(", ");
            format_sprs_value(&payload, out);
            out.push('}');
        }
        _ => {
            out.push_str("<unknown type>");
        }
    }
}

// ---------------------------------------------------------------------------
// C ABI: __panic
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __panic(message_ptr: *const i8) {
    if message_ptr.is_null() {
        sprs_out_line("Panic: <null message>");
    } else {
        let c_str = unsafe { CStr::from_ptr(message_ptr) };
        let message = c_str.to_string_lossy();
        let mut buf = String::from("Panic: ");
        buf.push_str(&message);
        sprs_out_line(&buf);
    }
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::{
        __atom_eq, __atom_from_bytes, __atom_from_string, __atom_name, __buffer_exist,
        __buffer_get, __buffer_into_raw, __buffer_len, __buffer_new, __buffer_set, __clone, __drop,
        __error_label_from_str, __error_message_from_label, __label_is_error, __label_name,
        __label_name_eq, __label_new, __label_new_from_string, __label_payload, __list_get,
        __list_new, __list_push, __list_set, __raw_free, __string_eq, __string_new,
        __struct_borrow, __struct_new, __struct_track_value, __value_to_string, INVALID_HANDLE,
        RAW_LAYOUTS, SlotData, SprsValue, Tag, atom_name, format_sprs_value, intern_atom,
        slot_with,
    };

    #[test]
    fn label_round_trips_and_clones_payload() {
        let name = b"ok";
        let handle = __label_new(name.as_ptr(), name.len() as i64, Tag::Integer as i32, 42);
        let value = SprsValue {
            tag: Tag::Label as i32,
            data: handle,
        };
        let mut output = String::new();
        format_sprs_value(&value, &mut output);
        assert_eq!(output, "{:ok, 42}");

        let cloned = __clone(value.tag, value.data);
        let mut cloned_output = String::new();
        format_sprs_value(&cloned, &mut cloned_output);
        assert_eq!(cloned_output, "{:ok, 42}");

        __drop(value.tag, value.data);
        __drop(cloned.tag, cloned.data);
    }

    #[test]
    fn label_with_unit_payload_prints_brace_form() {
        // Bare `:ok` is now an Atom; a Label always carries a payload, so a
        // Unit payload renders as `{:ok, ()}` rather than `:ok`.
        let name = b"ok";
        let handle = __label_new(name.as_ptr(), name.len() as i64, Tag::Unit as i32, 0);
        let value = SprsValue {
            tag: Tag::Label as i32,
            data: handle,
        };
        let mut output = String::new();
        format_sprs_value(&value, &mut output);
        assert_eq!(output, "{:ok, ()}");
        __drop(value.tag, value.data);
    }

    #[test]
    fn atom_intern_returns_stable_ids() {
        let first = intern_atom("ok");
        let second = intern_atom("ok");
        assert_eq!(first, second);
        let other = intern_atom("error");
        assert_ne!(first, other);
        assert_eq!(atom_name(first).as_deref(), Some("ok"));
        assert_eq!(atom_name(u32::MAX), None);
    }

    #[test]
    fn atom_from_bytes_and_format() {
        let id = __atom_from_bytes(b"ok".as_ptr(), 2);
        assert_ne!(id, u64::MAX);
        let value = SprsValue {
            tag: Tag::Atom as i32,
            data: id,
        };
        let mut output = String::new();
        format_sprs_value(&value, &mut output);
        assert_eq!(output, ":ok");
        // Invalid id renders as :<?>
        let bad = SprsValue {
            tag: Tag::Atom as i32,
            data: u64::MAX,
        };
        let mut bad_output = String::new();
        format_sprs_value(&bad, &mut bad_output);
        assert_eq!(bad_output, ":<?>");
    }

    #[test]
    fn atom_from_string_interps_slot_content() {
        let string_handle = __string_new(b"item".as_ptr(), 4);
        let id = __atom_from_string(string_handle);
        assert_ne!(id, u64::MAX);
        assert_eq!(atom_name(id as u32).as_deref(), Some("item"));
        // Non-string handle → invalid id.
        assert_eq!(__atom_from_string(INVALID_HANDLE), u64::MAX);
        __drop(Tag::String as i32, string_handle);
    }

    #[test]
    fn atom_eq_compares_ids() {
        let a = __atom_from_bytes(b"ok".as_ptr(), 2);
        let b = __atom_from_bytes(b"ok".as_ptr(), 2);
        let c = __atom_from_bytes(b"no".as_ptr(), 2);
        assert_eq!(__atom_eq(a, b), 1);
        assert_eq!(__atom_eq(a, c), 0);
    }

    #[test]
    fn atom_name_returns_new_string_slot() {
        let id = __atom_from_bytes(b"hello".as_ptr(), 5);
        let name_handle = __atom_name(id);
        let text = slot_with(name_handle, String::new(), |slot_data| match slot_data {
            SlotData::String(string_value) => string_value.clone(),
            _ => String::new(),
        });
        assert_eq!(text, "hello");
        __drop(Tag::String as i32, name_handle);
        // Invalid id → empty string slot, never INVALID.
        let empty_handle = __atom_name(u64::MAX);
        assert_ne!(empty_handle, INVALID_HANDLE);
        __drop(Tag::String as i32, empty_handle);
    }

    #[test]
    fn atom_clone_and_drop_are_noops() {
        let id = __atom_from_bytes(b"ok".as_ptr(), 2);
        let cloned = __clone(Tag::Atom as i32, id);
        assert_eq!(cloned.tag, Tag::Atom as i32);
        assert_eq!(cloned.data, id);
        __drop(Tag::Atom as i32, id); // must not panic
        __drop(Tag::Atom as i32, id); // double drop must not panic
    }
    #[test]
    fn dropping_label_releases_heap_payload() {
        let payload = b"payload";
        let string_handle = __string_new(payload.as_ptr(), payload.len() as i64);
        let name = b"value";
        let label_handle = __label_new(
            name.as_ptr(),
            name.len() as i64,
            Tag::String as i32,
            string_handle,
        );
        __drop(Tag::Label as i32, label_handle);
        let replacement = __string_new(payload.as_ptr(), payload.len() as i64);
        assert_ne!(replacement, string_handle);
        __drop(Tag::String as i32, replacement);
    }

    #[test]
    fn value_to_string_supports_int_bool_str() {
        let integer_string_handle = __value_to_string(Tag::Integer as i32, 10u64);
        let eq = __label_name_eq(
            // misuse: wrap via label_new_from_string to check string content indirectly
            {
                let handle_value =
                    __label_new_from_string(integer_string_handle, Tag::Unit as i32, 0);
                assert_eq!(__label_name_eq(handle_value, b"10".as_ptr(), 2), 1);
                __drop(Tag::Label as i32, handle_value);
                integer_string_handle
            },
            b"10".as_ptr(),
            2,
        );
        // Direct string content check via __label_name path:
        let name_s = __string_new(b"hi".as_ptr(), 2);
        assert_eq!(__value_to_string(Tag::String as i32, name_s) != 0, true);
        let true_string_handle = __value_to_string(Tag::Boolean as i32, 1);
        let false_string_handle = __value_to_string(Tag::Boolean as i32, 0);
        let true_label_handle = __label_new_from_string(true_string_handle, Tag::Unit as i32, 0);
        let false_label_handle = __label_new_from_string(false_string_handle, Tag::Unit as i32, 0);
        assert_eq!(__label_name_eq(true_label_handle, b"true".as_ptr(), 4), 1);
        assert_eq!(__label_name_eq(false_label_handle, b"false".as_ptr(), 5), 1);
        __drop(Tag::Label as i32, true_label_handle);
        __drop(Tag::Label as i32, false_label_handle);
        __drop(Tag::String as i32, integer_string_handle);
        __drop(Tag::String as i32, name_s);
        __drop(Tag::String as i32, true_string_handle);
        __drop(Tag::String as i32, false_string_handle);
        let _ = eq;
    }

    #[test]
    fn label_query_helpers_work() {
        let name = b"ok";
        let handle = __label_new(name.as_ptr(), name.len() as i64, Tag::Integer as i32, 42);
        assert_eq!(__label_name_eq(handle, name.as_ptr(), name.len() as i64), 1);
        assert_eq!(__label_name_eq(handle, b"no".as_ptr(), 2), 0);
        let payload = __label_payload(handle);
        assert_eq!(payload.tag, Tag::Integer as i32);
        assert_eq!(payload.data, 42);
        let name_handle = __label_name(handle);
        let named = __label_new_from_string(name_handle, Tag::Unit as i32, 0);
        assert_eq!(__label_name_eq(named, name.as_ptr(), name.len() as i64), 1);
        __drop(Tag::Label as i32, named);
        __drop(Tag::String as i32, name_handle);
        __drop(Tag::Label as i32, handle);
        let empty_name = __label_name(0);
        let empty_label = __label_new_from_string(empty_name, Tag::Unit as i32, 0);
        assert_eq!(__label_name_eq(empty_label, b"".as_ptr(), 0), 1);
        __drop(Tag::Label as i32, empty_label);
        __drop(Tag::String as i32, empty_name);
    }

    #[test]
    fn label_is_error_matches_error_named_label() {
        let err_handle = __label_new(b"error".as_ptr(), 5, Tag::Unit as i32, 0);
        let ok_handle = __label_new(b"ok".as_ptr(), 2, Tag::Unit as i32, 0);
        assert_eq!(__label_is_error(Tag::Label as i32, err_handle), 1);
        assert_eq!(__label_is_error(Tag::Label as i32, ok_handle), 0);
        // Non-label tags never count as errors.
        assert_eq!(__label_is_error(Tag::Integer as i32, err_handle), 0);
        __drop(Tag::Label as i32, err_handle);
        __drop(Tag::Label as i32, ok_handle);
    }

    #[test]
    fn error_label_from_str_builds_error_label_with_string_payload() {
        let handle = __error_label_from_str(b"division by zero".as_ptr(), 16);
        assert_ne!(handle, INVALID_HANDLE);
        assert_eq!(__label_is_error(Tag::Label as i32, handle), 1);
        assert_eq!(__label_name_eq(handle, b"error".as_ptr(), 5), 1);
        let payload = __label_payload(handle);
        assert_eq!(payload.tag, Tag::String as i32);
        // Clean up the label and its string payload.
        __drop(Tag::Label as i32, handle);
        __drop(Tag::String as i32, payload.data);
    }

    #[test]
    fn error_message_from_label_returns_reason_string() {
        // String payload: cloned back out.
        let str_err = __error_label_from_str(b"boom".as_ptr(), 4);
        let msg = __error_message_from_label(str_err);
        let text = slot_with(msg, String::new(), |slot_data| match slot_data {
            SlotData::String(string_value) => string_value.clone(),
            _ => String::new(),
        });
        assert_eq!(text, "boom");
        __drop(Tag::String as i32, msg);
        __drop(Tag::Label as i32, str_err);

        // Non-string payload: rendered via format_sprs_value (`:enoent`).
        let label_err = __label_new(b"error".as_ptr(), 5, Tag::Integer as i32, 42);
        let msg = __error_message_from_label(label_err);
        let text = slot_with(msg, String::new(), |slot_data| match slot_data {
            SlotData::String(string_value) => string_value.clone(),
            _ => String::new(),
        });
        assert_eq!(text, "42");
        __drop(Tag::String as i32, msg);
        __drop(Tag::Label as i32, label_err);

        // Non-error label: empty string, never INVALID.
        let ok = __label_new(b"ok".as_ptr(), 2, Tag::Unit as i32, 0);
        let msg = __error_message_from_label(ok);
        assert_ne!(msg, INVALID_HANDLE);
        let text = slot_with(msg, String::new(), |slot_data| match slot_data {
            SlotData::String(string_value) => string_value.clone(),
            _ => String::new(),
        });
        assert_eq!(text, "");
        __drop(Tag::String as i32, msg);
        __drop(Tag::Label as i32, ok);

        // Non-label handle: empty string as well.
        let msg = __error_message_from_label(0);
        assert_ne!(msg, INVALID_HANDLE);
        let text = slot_with(msg, String::new(), |slot_data| match slot_data {
            SlotData::String(string_value) => string_value.clone(),
            _ => String::new(),
        });
        assert_eq!(text, "");
        __drop(Tag::String as i32, msg);
    }

    #[test]
    fn string_eq_compares_contents_and_rejects_stale() {
        let a = super::slot_insert(SlotData::String("abc".to_string()));
        let b = super::slot_insert(SlotData::String("abc".to_string()));
        let c = super::slot_insert(SlotData::String("abd".to_string()));
        assert_eq!(__string_eq(a, b), 1);
        assert_eq!(__string_eq(a, c), 0);
        assert_eq!(__string_eq(a, INVALID_HANDLE), 0);
        assert_eq!(__string_eq(INVALID_HANDLE, b), 0);
        __drop(Tag::String as i32, a);
        __drop(Tag::String as i32, b);
        __drop(Tag::String as i32, c);
    }

    #[test]
    fn buffer_new_set_get_len_drop() {
        let handle = __buffer_new(4);
        assert_ne!(handle, INVALID_HANDLE);
        assert_eq!(__buffer_len(handle), 4);

        // New buffers are zero-initialized.
        let zero = __buffer_get(handle, 0);
        assert_eq!(zero.tag, Tag::Integer as i32);
        assert_eq!(zero.data, 0);

        __buffer_set(handle, 0, 10);
        __buffer_set(handle, 1, 20);
        let v0 = __buffer_get(handle, 0);
        assert_eq!(v0.tag, Tag::Integer as i32);
        assert_eq!(v0.data, 10);
        let v1 = __buffer_get(handle, 1);
        assert_eq!(v1.data, 20);
        assert_eq!(__buffer_len(handle), 4);

        // Byte values wrap at 8 bits.
        __buffer_set(handle, 2, 256);
        assert_eq!(__buffer_get(handle, 2).data, 0);
        __buffer_set(handle, 3, 300);
        assert_eq!(__buffer_get(handle, 3).data, 44);

        // OOB read → Unit sentinel; OOB write is a no-op (no panic).
        let oob = __buffer_get(handle, 4);
        assert_eq!(oob.tag, Tag::Unit as i32);
        __buffer_set(handle, 99, 1);
        assert_eq!(__buffer_get(handle, -1).tag, Tag::Unit as i32);

        // Non-buffer / stale handles report 0 / Unit.
        assert_eq!(__buffer_exist(handle), 1);
        assert_eq!(__buffer_exist(INVALID_HANDLE), 0);
        assert_eq!(__buffer_len(INVALID_HANDLE), 0);

        // Drop makes the handle stale; double drop must not panic.
        __drop(Tag::Buffer as i32, handle);
        assert_eq!(__buffer_exist(handle), 0);
        assert_eq!(__buffer_len(handle), 0);
        __drop(Tag::Buffer as i32, handle);
    }

    #[test]
    fn buffer_zero_size_and_clone() {
        let empty = __buffer_new(0);
        assert_ne!(empty, INVALID_HANDLE);
        assert_eq!(__buffer_len(empty), 0);
        __drop(Tag::Buffer as i32, empty);

        let handle = __buffer_new(2);
        __buffer_set(handle, 0, 7);
        __buffer_set(handle, 1, 8);
        let cloned = __clone(Tag::Buffer as i32, handle);
        assert_eq!(cloned.tag, Tag::Buffer as i32);
        assert_ne!(cloned.data, handle);
        assert_eq!(__buffer_len(cloned.data), 2);
        assert_eq!(__buffer_get(cloned.data, 0).data, 7);
        assert_eq!(__buffer_get(cloned.data, 1).data, 8);

        // Mutating the original does not affect the deep copy.
        __buffer_set(handle, 0, 99);
        assert_eq!(__buffer_get(cloned.data, 0).data, 7);

        // Negative size → INVALID_HANDLE.
        assert_eq!(__buffer_new(-1), INVALID_HANDLE);

        __drop(Tag::Buffer as i32, handle);
        __drop(Tag::Buffer as i32, cloned.data);
    }

    #[test]
    fn raw_ptr_roundtrip_frees_and_empties_layouts() {
        let handle = __buffer_new(2);
        __buffer_set(handle, 0, 7);
        let ptr = __buffer_into_raw(handle);
        assert_ne!(ptr, 0);
        assert_eq!(__buffer_exist(handle), 0);
        assert_eq!(__buffer_into_raw(handle), 0);
        __raw_free(ptr);
        RAW_LAYOUTS.with(|layouts| assert!(layouts.borrow().is_empty()));
    }

    #[test]
    fn raw_free_double_and_unknown_are_noops() {
        let handle = __buffer_new(2);
        let ptr = __buffer_into_raw(handle);
        assert_ne!(ptr, 0);
        __raw_free(ptr);
        __raw_free(ptr);
        __raw_free(0);
        __raw_free(0xDEAD_BEEF);
    }

    #[test]
    fn buffer_into_raw_rejects_non_buffer_and_empty() {
        assert_eq!(__buffer_into_raw(INVALID_HANDLE), 0);
        let string_handle = __string_new(b"abc".as_ptr(), 3);
        assert_eq!(__buffer_into_raw(string_handle), 0);
        __drop(Tag::String as i32, string_handle);
        let empty = __buffer_new(0);
        assert_eq!(__buffer_into_raw(empty), 0);
        __drop(Tag::Buffer as i32, empty);
    }

    #[test]
    fn struct_drop_invalidates_tracked_string() {
        let payload = b"owned";
        let string_handle = __string_new(payload.as_ptr(), payload.len() as i64);
        let struct_handle = __struct_new(8);
        let field_ptr = __struct_borrow(struct_handle);
        assert!(!field_ptr.is_null());
        unsafe {
            std::ptr::write(field_ptr as *mut u64, string_handle);
        }
        assert_eq!(
            __struct_track_value(
                struct_handle,
                field_ptr,
                Tag::String as i32,
                string_handle,
                1
            ),
            1
        );
        __drop(Tag::Struct as i32, struct_handle);
        let live = slot_with(string_handle, false, |_| true);
        assert!(!live);
    }

    #[test]
    fn struct_clone_deep_copies_tracked_string() {
        let payload = b"copy";
        let string_handle = __string_new(payload.as_ptr(), payload.len() as i64);
        let struct_handle = __struct_new(8);
        let field_ptr = __struct_borrow(struct_handle);
        unsafe {
            std::ptr::write(field_ptr as *mut u64, string_handle);
        }
        assert_eq!(
            __struct_track_value(
                struct_handle,
                field_ptr,
                Tag::String as i32,
                string_handle,
                1
            ),
            1
        );
        let cloned = __clone(Tag::Struct as i32, struct_handle);
        assert_eq!(cloned.tag, Tag::Struct as i32);
        let cloned_field = __struct_borrow(cloned.data);
        let cloned_string = unsafe { std::ptr::read(cloned_field as *const u64) };
        assert_ne!(cloned_string, string_handle);
        __drop(Tag::Struct as i32, struct_handle);
        let text = slot_with(cloned_string, String::new(), |slot_data| match slot_data {
            SlotData::String(value) => value.clone(),
            _ => String::new(),
        });
        assert_eq!(text, "copy");
        __drop(Tag::Struct as i32, cloned.data);
    }

    #[test]
    fn struct_track_rejects_out_of_range_pointer() {
        let struct_handle = __struct_new(8);
        let field_ptr = __struct_borrow(struct_handle);
        let bad = unsafe { field_ptr.add(64) };
        assert_eq!(
            __struct_track_value(struct_handle, bad, Tag::String as i32, 1, 1),
            0
        );
        assert_eq!(
            __struct_track_value(INVALID_HANDLE, field_ptr, Tag::String as i32, 1, 1),
            0
        );
        __drop(Tag::Struct as i32, struct_handle);
    }

    #[test]
    fn list_get_moves_values_and_leaves_unit() {
        let list = __list_new(2);
        __list_push(list, Tag::Integer as i32, 42);
        let payload = b"heap";
        let string_handle = __string_new(payload.as_ptr(), payload.len() as i64);
        __list_push(list, Tag::String as i32, string_handle);

        let first = __list_get(list, 0);
        assert_eq!(first.tag, Tag::Integer as i32);
        assert_eq!(first.data, 42);
        let first_again = __list_get(list, 0);
        assert_eq!(first_again.tag, Tag::Unit as i32);
        assert_eq!(first_again.data, 0);

        let taken = __list_get(list, 1);
        assert_eq!(taken.tag, Tag::String as i32);
        assert_eq!(taken.data, string_handle);
        let taken_again = __list_get(list, 1);
        assert_eq!(taken_again.tag, Tag::Unit as i32);
        assert_eq!(taken_again.data, 0);

        __drop(Tag::List as i32, list);
        let live = slot_with(string_handle, false, |_| true);
        assert!(live);
        __drop(Tag::String as i32, string_handle);
        let live_after = slot_with(string_handle, false, |_| true);
        assert!(!live_after);
    }

    #[test]
    fn list_set_replaces_element() {
        let list = __list_new(0);
        __list_push(list, Tag::Integer as i32, 1);
        __list_push(list, Tag::Integer as i32, 2);
        __list_set(list, 0, Tag::Integer as i32, 9);
        let first = __list_get(list, 0);
        assert_eq!(first.tag, Tag::Integer as i32);
        assert_eq!(first.data, 9);
        __drop(Tag::List as i32, list);
    }

    #[test]
    fn dropping_list_releases_nested_heap_values() {
        let payload = b"nested";
        let string_handle = __string_new(payload.as_ptr(), payload.len() as i64);
        let inner = __list_new(1);
        __list_push(inner, Tag::String as i32, string_handle);
        let outer = __list_new(1);
        __list_push(outer, Tag::List as i32, inner);

        __drop(Tag::List as i32, outer);
        let string_live = slot_with(string_handle, false, |_| true);
        assert!(!string_live);
        let inner_live = slot_with(inner, false, |_| true);
        assert!(!inner_live);

        let immediates = __list_new(2);
        __list_push(immediates, Tag::Integer as i32, 1);
        __list_push(immediates, Tag::Boolean as i32, 1);
        __drop(Tag::List as i32, immediates);
    }
}

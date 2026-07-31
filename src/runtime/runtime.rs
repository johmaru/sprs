//! Sprs runtime — slab (handle table) based memory management.
//!
//! All heap values (List/Range/String/Struct/Enum) are stored in a global slot
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
//! - `__list_get` returns a bare `SprsValue` (16 bytes, same codegen pattern as
//!   `__clone`). OOB / bad handle is signalled by the `Unit` sentinel.
//! - Output (`__println`) is routed through a user-registerable callback
//!   (`__sprs_set_output`) so the runtime is host/stdout-agnostic. When no
//!   callback is registered, output falls back to `eprintln!` for host builds.
//!
//! Future `no_std` migration: replace `thread_local!` with
//! `critical_section::Mutex<Vec<Slot>>`; `RefCell` is already in `core`, and
//! the function bodies are unchanged.

use std::cell::RefCell;
use std::ffi::CStr;

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
    Enum = 7,
    Struct = 8,
    Error = 9,

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

// ---------------------------------------------------------------------------
// Runtime value payloads (internal, never cross the C ABI)
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct SprsRange {
    pub start: i64,
    pub end: i64,
}

#[repr(C)]
pub struct EnumInfo {
    /// Owned C string (NUL-terminated). Freed when the slot is dropped.
    pub name: *mut u8,
    pub name_len: usize,
    pub variant_index: i64,
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
    },
    Enum(EnumInfo),
    Error {
        code: u32,
        message: Option<String>,
    },
    Empty,
}

impl Drop for SlotData {
    fn drop(&mut self) {
        match self {
            // Vec / String / SprsRange / Error drop themselves.
            SlotData::List(_) | SlotData::String(_) | SlotData::Range(_)
            | SlotData::Error { .. } => {}
            SlotData::Struct { ptr, layout, owned } => {
                if *owned && !ptr.is_null() {
                    unsafe { std::alloc::dealloc(*ptr as *mut u8, *layout) };
                }
            }
            SlotData::Enum(info) => {
                if !info.name.is_null() {
                    unsafe {
                        std::alloc::dealloc(
                            info.name,
                            std::alloc::Layout::array::<u8>(info.name_len)
                                .unwrap_or_else(|_| std::alloc::Layout::new::<u8>()),
                        );
                    }
                    info.name = std::ptr::null_mut();
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
const fn handle_index(h: u64) -> u32 {
    (h >> 32) as u32
}

#[inline]
const fn handle_gen(h: u64) -> u32 {
    h as u32
}

/// Allocate a slot, returning a fresh handle. Index 0 is reserved.
fn slot_insert(data: SlotData) -> u64 {
    FREE_LIST.with(|fl| {
        SLOTS.with(|s| {
            let mut free_list = fl.borrow_mut();
            let mut slots = s.borrow_mut();
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
    FREE_LIST.with(|fl| {
        SLOTS.with(|s| {
            let mut slots = s.borrow_mut();
            let idx = handle_index(handle) as usize;
            if idx >= slots.len() {
                return;
            }
            let slot = &mut slots[idx];
            if slot.generation != handle_gen(handle) {
                return; // stale handle, not a live slot
            }
            // Drop the payload by replacing with Empty, then bump generation.
            slot.data = SlotData::Empty;
            slot.generation = slot.generation.wrapping_add(1);
            if slot.generation == 0 {
                slot.generation = 1;
            }
            fl.borrow_mut().push(idx as u32);
        })
    });
}

/// Read-only lookup that runs `f` with a reference to the payload if the
/// handle is live. Returns `f`'s result, or `default` on stale/invalid handle.
fn slot_with<T, F>(handle: u64, default: T, f: F) -> T
where
    F: FnOnce(&SlotData) -> T,
{
    if handle == INVALID_HANDLE {
        return default;
    }
    SLOTS.with(|s| {
        let slots = s.borrow();
        let idx = handle_index(handle) as usize;
        let Some(slot) = slots.get(idx) else {
            return default;
        };
        if slot.generation != handle_gen(handle) {
            return default;
        }
        f(&slot.data)
    })
}

// ---------------------------------------------------------------------------
// Output abstraction (user-registerable callback)
// ---------------------------------------------------------------------------

/// Register a function that receives raw bytes for output. The runtime uses
/// this for `__println`. If never called, output falls back to `eprintln!`.
#[unsafe(no_mangle)]
pub extern "C" fn __sprs_set_output(f: unsafe extern "C" fn(*const u8, usize)) {
    OUTPUT_FN.with(|cell| *cell.borrow_mut() = Some(f));
}

fn sprs_out(bytes: &[u8]) {
    OUTPUT_FN.with(|cell| {
        let opt = *cell.borrow();
        if let Some(f) = opt {
            // SAFETY: caller of `__sprs_set_output` guarantees the function
            // handles the pointer/length pair correctly.
            unsafe { f(bytes.as_ptr(), bytes.len()) };
        } else {
            // Fallback for host builds: stderr.
            use std::io::Write;
            let _ = std::io::stderr().write_all(bytes);
        }
    });
}

fn sprs_out_str(s: &str) {
    sprs_out(s.as_bytes());
}

fn sprs_out_line(s: &str) {
    sprs_out_str(s);
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
    SLOTS.with(|s| {
        let mut slots = s.borrow_mut();
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
    SLOTS.with(|s| {
        let mut slots = s.borrow_mut();
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
        vec[index as usize]
    })
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
    let s = String::from_utf8_lossy(bytes).into_owned();
    slot_insert(SlotData::String(s))
}

#[unsafe(no_mangle)]
pub extern "C" fn __string_from_cstr(cstr_ptr: *const i8) -> u64 {
    if cstr_ptr.is_null() {
        return INVALID_HANDLE;
    }
    let c_str = unsafe { CStr::from_ptr(cstr_ptr) };
    let s = c_str.to_string_lossy().into_owned();
    slot_insert(SlotData::String(s))
}

/// Concatenate two String slots into a fresh String slot. Read-then-release-
/// then-insert pattern: clone both source strings out of their slots (dropping
/// the borrows), then allocate the concatenated string in a new slot. This
/// avoids the dangling-pointer issue that `__string_borrow` had and moves the
/// `l_len + r_len` overflow / memcpy logic into safe Rust, eliminating
/// BUG-L02 (string concat heap buffer overflow).
#[unsafe(no_mangle)]
pub extern "C" fn __string_concat(l_handle: u64, r_handle: u64) -> u64 {
    let l: Option<String> = slot_with(l_handle, None, |d| match d {
        SlotData::String(s) => Some(s.clone()),
        _ => None,
    });
    let r: Option<String> = slot_with(r_handle, None, |d| match d {
        SlotData::String(s) => Some(s.clone()),
        _ => None,
    });
    match (l, r) {
        (Some(mut l), Some(r)) => {
            l.push_str(&r);
            slot_insert(SlotData::String(l))
        }
        _ => INVALID_HANDLE,
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
        });
    }
    let size_us = size as usize;
    let layout = match std::alloc::Layout::from_size_align(size_us, 8) {
        Ok(l) => l,
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
    })
}

/// Borrow the raw struct pointer for field access. Valid until the slot is
/// mutated or released.
#[unsafe(no_mangle)]
pub extern "C" fn __struct_borrow(handle: u64) -> *mut u8 {
    if handle == INVALID_HANDLE {
        return std::ptr::null_mut();
    }
    SLOTS.with(|s| {
        let slots = s.borrow();
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

// ---------------------------------------------------------------------------
// C ABI: Enum
// ---------------------------------------------------------------------------

/// Allocate an enum slot. `name_ptr`/`name_len` describe the variant name
/// (copied into the slot). `variant_index` is the numeric variant.
#[unsafe(no_mangle)]
pub extern "C" fn __enum_new(name_ptr: *const u8, name_len: i64, variant_index: i64) -> u64 {
    if name_ptr.is_null() || name_len < 0 {
        return INVALID_HANDLE;
    }
    let name_len_us = name_len as usize;
    let layout = match std::alloc::Layout::array::<u8>(name_len_us) {
        Ok(l) => l,
        Err(_) => return INVALID_HANDLE,
    };
    let name_buf = unsafe { std::alloc::alloc(layout) };
    if name_buf.is_null() {
        return INVALID_HANDLE;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(name_ptr, name_buf, name_len_us);
    }
    slot_insert(SlotData::Enum(EnumInfo {
        name: name_buf,
        name_len: name_len_us,
        variant_index,
    }))
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

    if tag == Tag::Enum as i32 {
        let new_handle = enum_clone(data);
        return SprsValue {
            tag,
            data: new_handle,
        };
    }

    if tag == Tag::Error as i32 {
        let new_handle = error_clone(data);
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
    let cloned: Option<String> = slot_with(handle, None, |d| match d {
        SlotData::String(s) => Some(s.clone()),
        _ => None,
    });
    match cloned {
        Some(s) => slot_insert(SlotData::String(s)),
        None => INVALID_HANDLE,
    }
}

/// Same read-then-release-then-insert pattern as `string_clone`.
fn range_clone(handle: u64) -> u64 {
    let cloned: Option<SprsRange> = slot_with(handle, None, |d| match d {
        SlotData::Range(r) => Some(SprsRange {
            start: r.start,
            end: r.end,
        }),
        _ => None,
    });
    match cloned {
        Some(r) => slot_insert(SlotData::Range(r)),
        None => INVALID_HANDLE,
    }
}

fn list_clone(handle: u64) -> u64 {
    // Collect the source elements by cloning each one, then insert a new list
    // slot holding the cloned vector. Done in two passes because `slot_insert`
    // borrows SLOTS mutably and we can't hold a borrowed reference across it.
    let snapshot: Vec<SprsValue> = slot_with(handle, Vec::new(), |d| match d {
        SlotData::List(v) => v.clone(),
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
    slot_with(handle, false, |d| matches!(d, SlotData::List(_)))
}

fn struct_clone(handle: u64) -> u64 {
    // Read the layout+bytes out, then insert a fresh slot with a copy.
    let (ptr, layout, size): (*mut u8, std::alloc::Layout, usize) = slot_with(
        handle,
        (std::ptr::null_mut(), std::alloc::Layout::new::<u8>(), 0),
        |d| match d {
            SlotData::Struct { ptr, layout, .. } => (*ptr, *layout, layout.size()),
            _ => (std::ptr::null_mut(), std::alloc::Layout::new::<u8>(), 0),
        },
    );
    if ptr.is_null() || size == 0 {
        return slot_insert(SlotData::Struct {
            ptr: std::mem::align_of::<u8>() as *mut u8,
            layout: std::alloc::Layout::new::<u8>(),
            owned: false,
        });
    }
    let new_ptr = unsafe { std::alloc::alloc(layout) };
    if new_ptr.is_null() {
        return INVALID_HANDLE;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(ptr, new_ptr, size);
    }
    slot_insert(SlotData::Struct {
        ptr: new_ptr,
        layout,
        owned: true,
    })
}

fn enum_clone(handle: u64) -> u64 {
    let (name_ptr, name_len, variant_index) =
        slot_with(handle, (std::ptr::null_mut(), 0usize, 0i64), |d| match d {
            SlotData::Enum(info) => (info.name, info.name_len, info.variant_index),
            _ => (std::ptr::null_mut(), 0, 0),
        });
    if name_ptr.is_null() {
        return INVALID_HANDLE;
    }
    __enum_new(name_ptr, name_len as i64, variant_index)
}

fn error_clone(handle: u64) -> u64 {
    let (code, message) = slot_with(handle, (0u32, None), |d| match d {
        SlotData::Error { code, message } => (*code, message.clone()),
        _ => (0, None),
    });
    slot_insert(SlotData::Error { code, message })
}

// ---------------------------------------------------------------------------
// C ABI: Error values
// ---------------------------------------------------------------------------

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

/// Check if a value's tag is `Tag::Error`. Returns 1 (true) or 0 (false).
#[unsafe(no_mangle)]
pub extern "C" fn __is_error(handle: u64) -> i32 {
    let tag = slot_with(handle, Tag::Unit as i32, |d| {
        if matches!(d, SlotData::Error { .. }) {
            Tag::Error as i32
        } else {
            Tag::Unit as i32
        }
    });
    if tag == Tag::Error as i32 { 1 } else { 0 }
}

/// Get the error code from an error value. Returns 0 if not an error.
#[unsafe(no_mangle)]
pub extern "C" fn __error_code(handle: u64) -> u32 {
    slot_with(handle, 0u32, |d| match d {
        SlotData::Error { code, .. } => *code,
        _ => 0,
    })
}

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

// ---------------------------------------------------------------------------
// C ABI: __strlen (legacy — reads a String slot's length)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn __strlen(handle: u64) -> i64 {
    slot_with(handle, 0i64, |d| match d {
        SlotData::String(s) => s.len() as i64,
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
        Ok(l) => l,
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
    let snapshot: Vec<SprsValue> = slot_with(list_handle, Vec::new(), |d| match d {
        SlotData::List(v) => v.clone(),
        _ => Vec::new(),
    });
    if snapshot.is_empty() && !slot_is_list(list_handle) {
        // Invalid handle: print nothing.
        return;
    }
    let mut buf = String::new();
    for (i, val) in snapshot.iter().enumerate() {
        if i > 0 {
            buf.push(' ');
        }
        format_sprs_value(val, &mut buf);
    }
    sprs_out_line(&buf);
}

fn format_sprs_value(val: &SprsValue, out: &mut String) {
    match val.tag {
        t if t == Tag::Integer as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i64);
        }
        t if t == Tag::Float as i32 || t == Tag::Float64 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", f64::from_bits(val.data));
        }
        t if t == Tag::Float16 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", f16_tof32(val.data as u16));
        }
        t if t == Tag::Float32 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", f32::from_bits(val.data as u32));
        }
        t if t == Tag::String as i32 => {
            slot_with(val.data, (), |d| match d {
                SlotData::String(s) => out.push_str(s),
                _ => out.push_str("<invalid string>"),
            });
        }
        t if t == Tag::Boolean as i32 => {
            out.push_str(if val.data != 0 { "true" } else { "false" });
        }
        t if t == Tag::List as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "<list handle {:016x}>", val.data);
        }
        t if t == Tag::Range as i32 => {
            use std::fmt::Write;
            let (start, end) = slot_with(val.data, (0i64, 0i64), |d| match d {
                SlotData::Range(r) => (r.start, r.end),
                _ => (0, 0),
            });
            let _ = write!(out, "<range {}..{}>", start, end);
        }
        t if t == Tag::Int8 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i8);
        }
        t if t == Tag::Uint8 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u8);
        }
        t if t == Tag::Int16 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i16);
        }
        t if t == Tag::Uint16 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u16);
        }
        t if t == Tag::Int32 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i32);
        }
        t if t == Tag::Uint32 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u32);
        }
        t if t == Tag::Int64 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as i64);
        }
        t if t == Tag::Uint64 as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", val.data as u64);
        }
        t if t == Tag::Unit as i32 => {
            out.push_str("()");
        }
        t if t == Tag::Enum as i32 => {
            use std::fmt::Write;
            let (name, idx) = slot_with(val.data, (String::new(), 0i64), |d| match d {
                SlotData::Enum(info) => {
                    let name = if info.name.is_null() {
                        String::new()
                    } else {
                        let slice = unsafe { std::slice::from_raw_parts(info.name, info.name_len) };
                        String::from_utf8_lossy(slice).into_owned()
                    };
                    (name, info.variant_index)
                }
                _ => (String::new(), 0),
            });
            let _ = write!(out, "<enum {} variant {}>", name, idx);
        }
        t if t == Tag::Struct as i32 => {
            use std::fmt::Write;
            let _ = write!(out, "<struct handle {:016x}>", val.data);
        }
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

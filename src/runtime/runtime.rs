#[repr(C)]
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

fn f16_tof32(bit: u16) -> f32 {
    let sign = (bit >> 15) as u32;
    let exp = ((bit >> 10) & 0x1F) as u32;
    let mant = (bit & 0x3FF) as u32;

    if exp == 0 {
        if mant == 0 {
            f32::from_bits(sign << 31)
        } else {
            // Subnormal: (-1)^s * 0.mant * 2^-14
            // = (-1)^s * mant * 2^-14
            let val = mant as f32 / 16777216.0; // 2^24
            if sign == 1 { -val } else { val }
        }
    } else if exp == 31 {
        if mant == 0 {
            // Infinity
            f32::from_bits((sign << 31) | 0x7F800000) // Inf
        } else {
            // NaN
            f32::from_bits((sign << 31) | 0x7F800000 | (mant << 13)) // NaN
        }
    } else {
        // Normalized number
        let new_exp = exp + 112;
        f32::from_bits((sign << 31) | (new_exp << 23) | (mant << 13))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __list_new(capacity: i64) -> *mut Vec<SprsValue> {
    let vec = Vec::with_capacity(capacity as usize);
    Box::into_raw(Box::new(vec))
}

#[unsafe(no_mangle)]
pub extern "C" fn __list_push(list_ptr: *mut Vec<SprsValue>, tag: i32, data: u64) {
    let list = unsafe { &mut *list_ptr };
    list.push(SprsValue { tag, data });
}

#[unsafe(no_mangle)]
pub extern "C" fn __list_get(list_ptr: *mut Vec<SprsValue>, index: i64) -> *mut SprsValue {
    let list = unsafe { &mut *list_ptr };

    if index < 0 || (index as usize) >= list.len() {
        eprintln!("Index out of bounds: {}", index);
        std::process::exit(1);
    }
    &mut list[index as usize]
}

pub struct SprsRange {
    pub start: i64,
    pub end: i64,
}
#[unsafe(no_mangle)]
pub extern "C" fn __range_new(start: i64, end: i64) -> *mut SprsRange {
    let range = Box::new(SprsRange { start, end });
    Box::into_raw(range)
}

#[unsafe(no_mangle)]
pub extern "C" fn __println(list_ptr: *mut Vec<SprsValue>) {
    let list = unsafe { &*list_ptr };

    for (i, val) in list.iter().enumerate() {
        match val.tag {
            t if t == Tag::Integer as i32 => {
                // integer
                println!("{}", val.data as i64);
            }
            t if t == Tag::Float as i32 => {
                // float
                let float_bits = val.data;
                let float_value = f64::from_bits(float_bits);
                println!("{}", float_value);
            }
            t if t == Tag::Float16 as i32 => {
                // f16
                let float_bits = val.data as u16;
                let float_value = f16_tof32(float_bits);
                println!("{}", float_value);
            }
            t if t == Tag::Float32 as i32 => {
                // f32
                let float_bits = val.data as u32;
                let float_value = f32::from_bits(float_bits);
                println!("{}", float_value);
            }
            t if t == Tag::Float64 as i32 => {
                // f64
                let float_bits = val.data;
                let float_value = f64::from_bits(float_bits);
                println!("{}", float_value);
            }
            t if t == Tag::String as i32 => {
                // string
                let c_str = unsafe { std::ffi::CStr::from_ptr(val.data as *const i8) };
                println!("{}", c_str.to_string_lossy());
            }
            t if t == Tag::Boolean as i32 => {
                // boolean
                let bool_str = if val.data != 0 { "true" } else { "false" };
                println!("{}", bool_str);
            }
            t if t == Tag::List as i32 => {
                // list
                println!(
                    "Value[{}]: <list at {:p}>",
                    i, val.data as *mut Vec<SprsValue>
                );
            }
            t if t == Tag::Range as i32 => {
                // range
                let range_ptr = val.data as *mut SprsRange;
                let range = unsafe { &*range_ptr };
                println!("Value[{}]: <range {}..{}>", i, range.start, range.end);
            }
            t if t == Tag::Int8 as i32 => {
                // i8
                println!("{}", val.data as i8);
            }
            t if t == Tag::Uint8 as i32 => {
                // u8
                println!("{}", val.data as u8);
            }
            t if t == Tag::Int16 as i32 => {
                // i16
                println!("{}", val.data as i16);
            }
            t if t == Tag::Uint16 as i32 => {
                // u16
                println!("{}", val.data as u16);
            }
            t if t == Tag::Int32 as i32 => {
                // i32
                println!("{}", val.data as i32);
            }
            t if t == Tag::Uint32 as i32 => {
                // u32
                println!("{}", val.data as u32);
            }
            t if t == Tag::Int64 as i32 => {
                // i64
                println!("{}", val.data as i64);
            }
            t if t == Tag::Uint64 as i32 => {
                // u64
                println!("{}", val.data as u64);
            }
            t if t == Tag::Unit as i32 => {
                // unit
                println!("Value[{}]: ()", i);
            }
            _ => {
                println!("Value[{}]: <unknown type>", i);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __strlen(s_ptr: *const i8) -> i64 {
    let c_str = unsafe { std::ffi::CStr::from_ptr(s_ptr) };
    c_str.to_bytes().len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn __malloc(size: i64) -> *mut i8 {
    let layout = std::alloc::Layout::from_size_align(size as usize, 8).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    ptr as *mut i8
}

#[unsafe(no_mangle)]
pub extern "C" fn __drop(val: SprsValue) {
    match val.tag {
        t if t == Tag::List as i32 => {
            let ptr = val.data as *mut Vec<SprsValue>;
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr);
                }
            }
        }
        t if t == Tag::Range as i32 => {
            let ptr = val.data as *mut SprsRange;
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr);
                }
            }
        }
        _ => {}
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __clone(tag: i32, data: u64) -> SprsValue {
    match tag {
        t if t == Tag::Integer as i32 => SprsValue { tag, data },
        t if t == Tag::Float as i32 => SprsValue { tag, data },
        t if t == Tag::Float16 as i32 => SprsValue { tag, data },
        t if t == Tag::Float32 as i32 => SprsValue { tag, data },
        t if t == Tag::Float64 as i32 => SprsValue { tag, data },
        t if t == Tag::Boolean as i32 => SprsValue { tag, data },
        t if t == Tag::String as i32 => {
            let c_str = unsafe { std::ffi::CStr::from_ptr(data as *const i8) };
            let bytes = c_str.to_bytes();
            let layout = std::alloc::Layout::from_size_align(bytes.len(), 1).unwrap();
            let ptr = unsafe { std::alloc::alloc(layout) };
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            }
            SprsValue {
                tag,
                data: ptr as u64,
            }
        }
        t if t == Tag::List as i32 => {
            let src_vec = unsafe { &*(data as *mut Vec<SprsValue>) };
            let mut new_vec = Vec::with_capacity(src_vec.len());
            for val in src_vec {
                new_vec.push(__clone(val.tag, val.data));
            }
            SprsValue {
                tag,
                data: Box::into_raw(Box::new(new_vec)) as u64,
            }
        }
        t if t == Tag::Range as i32 => {
            let src_range = unsafe { &*(data as *mut SprsRange) };
            let new_range = Box::new(SprsRange {
                start: src_range.start,
                end: src_range.end,
            });
            SprsValue {
                tag,
                data: Box::into_raw(new_range) as u64,
            }
        }
        _ => SprsValue { tag, data },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __panic(message_ptr: *const i8) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(message_ptr) };
    let message = c_str.to_string_lossy();
    eprintln!("Panic: {}", message);
    std::process::exit(1);
}

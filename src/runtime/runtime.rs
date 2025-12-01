
#[repr(C)]
pub struct SprsValue {
    pub tag: i32,
    pub data: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn __list_new(capacity: i64) -> *mut Vec<SprsValue> {
    let vec = Vec::with_capacity(capacity as usize);
    Box::into_raw(Box::new(vec))
}

#[unsafe(no_mangle)]
pub extern "C" fn __list_push(list_ptr: *mut Vec<SprsValue>, tag: i32, data: u64) {
    let list = unsafe {
        &mut *list_ptr
    };
    list.push(SprsValue { tag, data });
}

#[unsafe(no_mangle)]
pub extern "C" fn __list_get(list_ptr: *mut Vec<SprsValue>, index: i64) -> *mut SprsValue {
    let list = unsafe {
        &mut *list_ptr
    };

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
    let list = unsafe {
        &*list_ptr
    };

    for (i, val) in list.iter().enumerate() {
        match val.tag {
            0 => { // integer
                println!("{}",val.data as i64);
            },
            1 => { // string
                let c_str = unsafe { std::ffi::CStr::from_ptr(val.data as *const i8) };
                println!("{}", c_str.to_string_lossy());
            },
            2 => { // boolean
                let bool_str = if val.data != 0 { "true" } else { "false" };
                println!("{}", bool_str);
            },
            3 => { // list
                println!("Value[{}]: <list at {:p}>", i, val.data as *mut Vec<SprsValue>);
            },
            4 => { // range
                let range_ptr = val.data as *mut SprsRange;
                let range = unsafe { &*range_ptr };
                println!("Value[{}]: <range {}..{}>", i, range.start, range.end);
            },
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
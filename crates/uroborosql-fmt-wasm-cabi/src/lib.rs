//! Build uroborosql-fmt as a wasm module with a tiny, plain C-ABI.
//!
//! It does NOT use wasm-bindgen or emscripten, so it needs no JavaScript glue.
//! Data crosses the boundary through the wasm linear memory, using only numbers
//! (a pointer and a length). The module has zero imports, so any host
//! (for example a JVM wasm runtime like Chicory) can call it directly.
//!
//! # Exported functions
//! - `alloc(size) -> ptr`  : reserve memory for the input
//! - `dealloc(ptr, size)`  : free memory that `alloc` returned
//! - `format(src_ptr, src_len, cfg_ptr, cfg_len) -> ptr` : format SQL and return a result buffer
//! - `free_result(ptr)`    : free the buffer that `format` returned
//!
//! # Result buffer layout (the rule the host must follow)
//! ```text
//! offset 0 : u32 LE  cap     ... total size of this buffer (free_result needs it)
//! offset 4 : u8      status  ... 0 = OK, 1 = error
//! offset 5 : u32 LE  len     ... length of body in bytes
//! offset 9 : u8[len] body    ... UTF-8 text: the formatted SQL when OK, an error message when error
//! ```
//!
//! # Build
//! ```sh
//! cargo build -p uroborosql-fmt-wasm-cabi --target wasm32-unknown-unknown --release
//! ```

use std::alloc::{alloc as sys_alloc, dealloc as sys_dealloc, Layout};

use uroborosql_fmt::format_sql;

/// Size of the result buffer header: cap(4) + status(1) + len(4).
const HEADER: usize = 9;

/// Reserve memory in the wasm linear memory so the host can write input text.
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let layout = Layout::from_size_align(size, 1).expect("invalid layout");
    // SAFETY: size is greater than 0 (checked above).
    unsafe { sys_alloc(layout) }
}

/// Free memory that `alloc` returned.
///
/// # Safety
/// `ptr` must be a pointer returned by `alloc`, and `size` must be the same
/// value that was passed to that `alloc` call.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    if ptr.is_null() || size == 0 {
        return;
    }
    let layout = Layout::from_size_align(size, 1).expect("invalid layout");
    sys_dealloc(ptr, layout);
}

/// Format the SQL and return a pointer to the result buffer.
///
/// # Safety
/// `(src_ptr, src_len)` and `(cfg_ptr, cfg_len)` must each describe a valid
/// range inside the wasm linear memory, or be `(null, 0)`.
#[no_mangle]
pub unsafe extern "C" fn format(
    src_ptr: *const u8,
    src_len: usize,
    cfg_ptr: *const u8,
    cfg_len: usize,
) -> *mut u8 {
    let src = match slice_to_str(src_ptr, src_len) {
        Ok(s) => s,
        Err(_) => return pack(1, b"invalid UTF-8 in SQL input"),
    };

    // An empty or whitespace-only config means "use the default settings" (None).
    let cfg: Option<&str> = match slice_to_str(cfg_ptr, cfg_len) {
        Ok(s) if !s.trim().is_empty() => Some(s),
        Ok(_) => None,
        Err(_) => return pack(1, b"invalid UTF-8 in config JSON"),
    };

    match format_sql(src, cfg, None) {
        Ok(out) => pack(0, out.as_bytes()),
        Err(e) => pack(1, e.to_string().as_bytes()),
    }
}

/// Free the buffer that `format` returned.
///
/// # Safety
/// `ptr` must be a pointer returned by `format` that has not been freed yet.
#[no_mangle]
pub unsafe extern "C" fn free_result(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    // The first 4 bytes hold `cap` (pack wrote it there).
    let mut cap_bytes = [0u8; 4];
    std::ptr::copy_nonoverlapping(ptr, cap_bytes.as_mut_ptr(), 4);
    let cap = u32::from_le_bytes(cap_bytes) as usize;
    if cap < HEADER {
        return;
    }
    let layout = Layout::from_size_align(cap, 1).expect("invalid layout");
    sys_dealloc(ptr, layout);
}

/// Turn (ptr, len) into a `&str`. Returns an empty string if ptr is null or len is 0.
///
/// # Safety
/// The caller must make sure (ptr, len) is a valid range inside the linear memory.
unsafe fn slice_to_str<'a>(ptr: *const u8, len: usize) -> Result<&'a str, std::str::Utf8Error> {
    if ptr.is_null() || len == 0 {
        return Ok("");
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    std::str::from_utf8(slice)
}

/// Put `status` and `body` into a result buffer (see the layout in the module doc)
/// and return the pointer to its start.
fn pack(status: u8, body: &[u8]) -> *mut u8 {
    let cap = HEADER + body.len();
    let layout = Layout::from_size_align(cap, 1).expect("invalid layout");
    // SAFETY: cap >= HEADER > 0. Every write below stays inside the allocated buffer.
    unsafe {
        let p = sys_alloc(layout);
        if p.is_null() {
            return std::ptr::null_mut();
        }
        // cap (u32 LE)
        std::ptr::copy_nonoverlapping((cap as u32).to_le_bytes().as_ptr(), p, 4);
        // status
        *p.add(4) = status;
        // len (u32 LE)
        std::ptr::copy_nonoverlapping((body.len() as u32).to_le_bytes().as_ptr(), p.add(5), 4);
        // body
        if !body.is_empty() {
            std::ptr::copy_nonoverlapping(body.as_ptr(), p.add(HEADER), body.len());
        }
        p
    }
}

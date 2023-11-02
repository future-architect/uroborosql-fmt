use once_cell::sync::Lazy;
use std::{
    ffi::{c_char, CStr, CString},
    sync::Mutex,
};

static RESULT: Lazy<Mutex<CString>> = Lazy::new(|| Mutex::new(CString::new("").unwrap()));
static ERROR_MSG: Lazy<Mutex<CString>> = Lazy::new(|| Mutex::new(CString::new("").unwrap()));

use uroborosql_fmt::{config::Config, format_sql_with_config};

/// Returns the address of the result string.
///
/// # Safety
///
/// This is unsafe because it returns a raw pointer.
#[no_mangle]
pub unsafe extern "C" fn get_result_address() -> *const c_char {
    RESULT.lock().unwrap().as_c_str().as_ptr()
}

/// Returns the address of the error message string.
///
/// # Safety
///
/// This is unsafe because it returns a raw pointer.
#[no_mangle]
pub unsafe extern "C" fn get_error_msg_address() -> *const c_char {
    ERROR_MSG.lock().unwrap().as_c_str().as_ptr()
}

/// Formats SQL code given as char pointer `src` by WASM (JavaScript).
///
/// # Safety
///
/// This is unsafe because it uses unsafe function
/// [`CStr::from_ptr`](https://doc.rust-lang.org/stable/std/ffi/struct.CStr.html#method.from_ptr).
#[export_name = "format_sql"]
#[no_mangle]
pub unsafe extern "C" fn format_sql_for_wasm(src: *const c_char, config_json_str: *const c_char) {
    // Clear previous format result
    *RESULT.lock().unwrap() = CString::new("").unwrap();
    *ERROR_MSG.lock().unwrap() = CString::new("").unwrap();

    let src = CStr::from_ptr(src).to_str().unwrap().to_owned();

    let settings_json = CStr::from_ptr(config_json_str).to_str().unwrap();
    let config = Config::new(Some(settings_json), None).unwrap();

    let result = format_sql_with_config(&src, config);

    match result {
        Ok(result) => *RESULT.lock().unwrap() = CString::new(result).unwrap(),
        Err(err) => *ERROR_MSG.lock().unwrap() = CString::new(err.to_string()).unwrap(),
    }
}

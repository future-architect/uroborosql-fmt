use std::ffi::{c_char, CStr, CString};

use uroborosql_fmt::{config::Config, format_sql_with_config};

#[export_name = "format_sql"]
#[no_mangle]
pub extern "C" fn format_sql_for_wasm(
    src: *mut c_char,
    config_json_str: *mut c_char,
) -> *mut c_char {
    let src = unsafe { CStr::from_ptr(src).to_str().unwrap().to_owned() };

    let config_json_str = unsafe { CStr::from_ptr(config_json_str).to_str().unwrap() };
    let config = Config::from_json_str(config_json_str).unwrap();

    // TODO: error handling
    let result = format_sql_with_config(&src, config).unwrap();

    CString::new(result).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn free_format_string(s: *mut c_char) {
    unsafe {
        if s.is_null() {
            return;
        }
        CString::from_raw(s)
    };
}

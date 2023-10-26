use std::ffi::{c_char, CStr, CString};

static mut RESULT: &mut [u8] = &mut [0; 50000];
static mut ERROR_MSG: &mut [u8] = &mut [0; 50000];

use uroborosql_fmt::{config::Config, format_sql_with_config};

/// Returns the address of the result string.
///
/// # Safety
///
/// This is unsafe because it returns a raw pointer.
#[no_mangle]
pub unsafe extern "C" fn get_result_address() -> *const u8 {
    &RESULT[0]
}

/// Returns the address of the error message string.
///
/// # Safety
///
/// This is unsafe because it returns a raw pointer.
#[no_mangle]
pub unsafe extern "C" fn get_error_msg_address() -> *const u8 {
    &ERROR_MSG[0]
}

/// Formats SQL code given as char pointer `src` by WASM (JavaScript).
///
/// # Safety
///
/// This is unsafe because it uses unsafe function
/// [`CStr::from_ptr`](https://doc.rust-lang.org/stable/std/ffi/struct.CStr.html#method.from_ptr).
#[export_name = "format_sql"]
#[no_mangle]
pub unsafe extern "C" fn format_sql_for_wasm(src: *mut c_char, config_json_str: *mut c_char) {
    let src = CStr::from_ptr(src).to_str().unwrap().to_owned();

    let config_json_str = CStr::from_ptr(config_json_str).to_str().unwrap();
    let config = Config::from_json_str(config_json_str).unwrap();

    let result = format_sql_with_config(&src, config);

    match result {
        Ok(result) => {
            CString::new(result)
                .unwrap()
                .as_bytes_with_nul()
                .iter()
                .enumerate()
                .for_each(|(i, x)| {
                    RESULT[i] = *x;
                });

            CString::new("")
                .unwrap()
                .as_bytes_with_nul()
                .iter()
                .enumerate()
                .for_each(|(i, x)| {
                    ERROR_MSG[i] = *x;
                });
        }
        Err(err) => {
            CString::new(err.to_string())
                .unwrap()
                .as_bytes_with_nul()
                .iter()
                .enumerate()
                .for_each(|(i, x)| {
                    ERROR_MSG[i] = *x;
                });

            CString::new("")
                .unwrap()
                .as_bytes_with_nul()
                .iter()
                .enumerate()
                .for_each(|(i, x)| {
                    RESULT[i] = *x;
                });
        }
    }
}

use uroborosql_fmt::format_sql;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn format_sql_for_wasm(src: &str, config_json_str: &str) -> Result<String, String> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    format_sql(src, Some(config_json_str), None).map_err(|e| e.to_string())
}

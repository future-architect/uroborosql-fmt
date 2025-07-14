mod utils;

use uroborosql_fmt::format_sql;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn format_sql_for_wasm(src: &str, config_json_str: &str) -> String {
    let result = format_sql(src, Some(config_json_str), None);

    match result {
        Ok(result) => result,
        Err(err) => err.to_string(),
    }
}

#![deny(clippy::all)]

use napi::{Error, Result, Status};
use uroborosql_fmt::{format_sql, format_sql_with_settings_json};

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn runfmt(input: String, config_path: Option<&str>) -> Result<String> {
  let result = format_sql(&input, config_path);

  match result {
    Ok(res) => Ok(res),
    Err(e) => Err(Error::new(Status::GenericFailure, format!("{e}"))),
  }
}

#[napi]
pub fn runfmt_with_settings(
  input: String,
  settings_json: String,
  config_path: Option<&str>,
) -> Result<String> {
  format_sql_with_settings_json(&input, &settings_json, config_path)
    .or_else(|e| Err(Error::new(Status::GenericFailure, format!("{e}"))))
}

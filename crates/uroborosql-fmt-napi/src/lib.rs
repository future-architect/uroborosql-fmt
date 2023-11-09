#![deny(clippy::all)]

use napi::{Error, Result, Status};
use uroborosql_fmt::format_sql;

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn runfmt(input: String, config_path: Option<&str>) -> Result<String> {
  let result = format_sql(&input, None, config_path);

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
  format_sql(&input, Some(&settings_json), config_path)
    .map_err(|e| Error::new(Status::GenericFailure, format!("{e}")))
}

#![deny(clippy::all)]

use napi::{Error, Result, Status};
use tokio::runtime::Runtime;
use uroborosql_fmt::format_sql;

#[macro_use]
extern crate napi_derive;

fn generic_error<E: std::fmt::Display>(err: E) -> Error {
  Error::new(Status::GenericFailure, format!("{err}"))
}

#[napi]
pub fn runfmt(input: String, config_path: Option<&str>) -> Result<String> {
  let result = format_sql(&input, None, config_path);

  match result {
    Ok(res) => Ok(res),
    Err(e) => Err(generic_error(e)),
  }
}

#[napi]
pub fn runfmt_with_settings(
  input: String,
  settings_json: String,
  config_path: Option<&str>,
) -> Result<String> {
  format_sql(&input, Some(&settings_json), config_path).map_err(generic_error)
}

#[napi]
pub fn run_language_server() -> Result<()> {
  let runtime = Runtime::new().map_err(generic_error)?;
  runtime.block_on(async { uroborosql_language_server::run_stdio().await });
  Ok(())
}

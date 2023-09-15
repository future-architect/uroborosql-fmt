#![deny(clippy::all)]

use napi::{Error, Result, Status};
use uroborosql_fmt::format_sql;

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn runfmt(input: String, config_path: Option<&str>) -> Result<String> {
  let result = format_sql(&input, config_path);

  match result {
    Ok(res) => Ok(res),
    Err(e) => Err(Error::new(Status::GenericFailure, format!("{}", e))),
  }
}

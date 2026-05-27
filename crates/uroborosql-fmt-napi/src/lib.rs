#![deny(clippy::all)]

use napi::{Error, Result, Status};

#[macro_use]
extern crate napi_derive;

#[napi]
pub fn run_language_server() -> Result<()> {
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|e| Error::new(Status::GenericFailure, format!("{e}")))?;

  runtime.block_on(uroborosql_language_server::run_stdio());
  Ok(())
}

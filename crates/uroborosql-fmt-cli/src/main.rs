use clap::Parser;
use std::process::exit;

mod cli;

use cli::{run, Cli, ExitCode};

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => exit(ExitCode::Ok as i32),
        Err(code) => exit(code as i32),
    }
}

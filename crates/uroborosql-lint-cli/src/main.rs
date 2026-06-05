use std::process;

use clap::Parser;

mod app;
mod args;

use app::{run, ExitCode};
use args::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        err.print();
        process::exit(err.exit_code() as i32);
    }

    process::exit(ExitCode::Ok as i32);
}

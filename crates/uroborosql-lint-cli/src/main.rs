use std::process;

use clap::Parser;

mod app;
mod args;

use app::run;
use args::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        err.print();
        process::exit(err.exit_code() as i32);
    }
}

use std::process;

use clap::Parser;

mod app;
mod args;

use app::run;
use args::Cli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> process::ExitCode {
    let cli = Cli::parse();

    match run(cli).await {
        Ok(()) => process::ExitCode::SUCCESS,
        Err(err) => {
            err.print();
            process::ExitCode::from(err.exit_code() as u8)
        }
    }
}

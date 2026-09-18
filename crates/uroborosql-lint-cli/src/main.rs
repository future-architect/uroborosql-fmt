use std::process;

use clap::Parser;

mod app;
mod args;
mod export;

use app::run;
use args::Cli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> process::ExitCode {
    let mut cli = Cli::parse();
    if let Some(args::Command::ExportCatalog(args)) = cli.command.take() {
        return process::ExitCode::from(export::run(args).await);
    }

    match run(cli).await {
        Ok(()) => process::ExitCode::SUCCESS,
        Err(err) => {
            err.print();
            process::ExitCode::from(err.exit_code() as u8)
        }
    }
}

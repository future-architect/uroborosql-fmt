use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "uroborosql-language-server",
    version,
    about = "Language server for SQL formatting and linting"
)]
struct Cli;

#[cfg(feature = "runtime-tokio")]
#[tokio::main]
async fn main() {
    let _ = Cli::parse();
    uroborosql_language_server::run_stdio().await;
}

#[cfg(not(feature = "runtime-tokio"))]
fn main() {
    panic!("runtime-tokio feature is required for stdio mode");
}

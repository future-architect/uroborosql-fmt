#[cfg(feature = "runtime-tokio")]
#[tokio::main]
async fn main() {
    uroborosql_language_server::run_stdio().await;
}

#[cfg(not(feature = "runtime-tokio"))]
fn main() {
    panic!("binary entrypoint requires the `runtime-tokio` feature");
}

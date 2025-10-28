#[tokio::main]
async fn main() {
    uroborosql_language_server::run_stdio().await;
}

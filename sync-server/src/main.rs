#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    geecko_sync_server::serve_from_environment().await
}

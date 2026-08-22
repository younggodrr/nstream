use std::env;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize structured tracing.
    nstream_observe::init_tracing();

    let port = env::var("API_PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    info!(addr = %addr, "Starting NStream API");

    if let Err(e) = nstream_api::run(&addr).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}

use nstream_api::state::{start_workers, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();
    let state = AppState::new().await?;
    start_workers(state);
    tokio::signal::ctrl_c().await?;
    Ok(())
}

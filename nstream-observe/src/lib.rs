pub mod cost;
pub mod metrics;

pub use cost::CostTracker;
pub use metrics::{register, NStreamMetrics};

use tracing_subscriber::{fmt, EnvFilter};

/// Initialize the global tracing subscriber with JSON-formatted output.
///
/// Reads the `RUST_LOG` environment variable for filter configuration.
/// Defaults to `info` if `RUST_LOG` is not set.
///
/// Should be called once at application startup before any `tracing` macros fire.
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .json()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    tracing::info!("Tracing initialized (JSON format)");
}

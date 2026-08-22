use thiserror::Error;

/// Top-level error type for all NStream operations.
#[derive(Debug, Error)]
pub enum NStreamError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Channel send error: {0}")]
    ChannelSend(String),

    #[error("Join error: {0}")]
    Join(String),
}

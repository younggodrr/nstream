use crate::{MediaPayload, NStreamError};
use async_trait::async_trait;

/// A processing node in the NStream pipeline DAG.
/// Each node receives a `MediaPayload`, transforms it, and returns the result.
#[async_trait]
pub trait Node: Send + Sync {
    /// Returns a human-readable name for this node.
    fn name(&self) -> &str;

    /// Process an incoming media payload and produce an outgoing payload.
    async fn process(&self, input: MediaPayload) -> Result<MediaPayload, NStreamError>;
}

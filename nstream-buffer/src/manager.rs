use bytes::Bytes;
use tracing::{debug, info};

use crate::disk_buffer::DiskBuffer;
use crate::ring_buffer::RingBuffer;
use nstream_core::{MediaPayload, NStreamError, PipelineMode};

/// Manages either an in-memory ring buffer (live mode) or a disk-backed buffer (batch mode).
pub struct BufferManager {
    mode: PipelineMode,
    ring: Option<RingBuffer<MediaPayload>>,
    disk: Option<DiskBuffer>,
}

impl BufferManager {
    /// Create a `BufferManager` configured for the given pipeline mode.
    pub fn new(mode: PipelineMode) -> Self {
        match mode {
            PipelineMode::LiveStream => {
                info!("BufferManager: using in-memory ring buffer (LiveStream mode)");
                Self {
                    mode,
                    ring: Some(RingBuffer::new()),
                    disk: None,
                }
            }
            PipelineMode::StudioBatch | PipelineMode::CreatorSelective => {
                info!(mode = %mode, "BufferManager: using disk-backed buffer");
                Self {
                    mode,
                    ring: None,
                    disk: Some(DiskBuffer::with_system_temp()),
                }
            }
        }
    }

    /// Push a `MediaPayload` into the active buffer.
    /// For live mode this is synchronous; for disk mode it is async.
    pub async fn push(&mut self, payload: MediaPayload) -> Result<(), NStreamError> {
        match self.mode {
            PipelineMode::LiveStream => {
                let ring = self
                    .ring
                    .as_mut()
                    .expect("ring buffer initialized for live mode");
                debug!(
                    len = ring.len(),
                    is_full = ring.is_full(),
                    "Pushing to ring buffer"
                );
                ring.push(payload);
                Ok(())
            }
            PipelineMode::StudioBatch | PipelineMode::CreatorSelective => {
                let disk = self
                    .disk
                    .as_mut()
                    .expect("disk buffer initialized for batch mode");
                // Serialize the payload to bytes for disk storage.
                let bytes = serialize_payload(&payload)?;
                debug!(bytes = bytes.len(), "Pushing to disk buffer");
                disk.push(bytes).await
            }
        }
    }

    /// Pop the oldest `MediaPayload` from the active buffer.
    pub async fn pop(&mut self) -> Result<Option<MediaPayload>, NStreamError> {
        match self.mode {
            PipelineMode::LiveStream => {
                let ring = self
                    .ring
                    .as_mut()
                    .expect("ring buffer initialized for live mode");
                Ok(ring.pop_front())
            }
            PipelineMode::StudioBatch | PipelineMode::CreatorSelective => {
                let disk = self
                    .disk
                    .as_mut()
                    .expect("disk buffer initialized for batch mode");
                match disk.pop().await {
                    Some(bytes) => {
                        let payload = deserialize_payload(bytes)?;
                        Ok(Some(payload))
                    }
                    None => Ok(None),
                }
            }
        }
    }

    /// Returns `true` if the active buffer is empty.
    pub fn is_empty(&self) -> bool {
        match self.mode {
            PipelineMode::LiveStream => self.ring.as_ref().map(|r| r.is_empty()).unwrap_or(true),
            _ => self.disk.as_ref().map(|d| d.is_empty()).unwrap_or(true),
        }
    }

    /// Returns the number of items currently buffered.
    pub fn len(&self) -> usize {
        match self.mode {
            PipelineMode::LiveStream => self.ring.as_ref().map(|r| r.len()).unwrap_or(0),
            _ => self.disk.as_ref().map(|d| d.len()).unwrap_or(0),
        }
    }

    /// Returns the current pipeline mode.
    pub fn mode(&self) -> &PipelineMode {
        &self.mode
    }
}

/// Serialize a `MediaPayload` to raw bytes using JSON.
fn serialize_payload(payload: &MediaPayload) -> Result<Bytes, NStreamError> {
    let json = serde_json::to_vec(payload)?;
    Ok(Bytes::from(json))
}

/// Deserialize a `MediaPayload` from raw bytes.
fn deserialize_payload(bytes: Bytes) -> Result<MediaPayload, NStreamError> {
    let payload: MediaPayload = serde_json::from_slice(&bytes)
        .map_err(|e| NStreamError::Decode(format!("Failed to deserialize payload: {}", e)))?;
    Ok(payload)
}

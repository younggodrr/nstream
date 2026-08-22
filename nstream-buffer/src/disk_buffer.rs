use bytes::Bytes;
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, error, info};
use uuid::Uuid;

use nstream_core::NStreamError;

/// Disk-backed buffer for overflow or batch mode.
/// Each frame is written as a separate temp file; paths are queued for retrieval.
pub struct DiskBuffer {
    /// Queue of file paths for buffered frames.
    file_queue: VecDeque<PathBuf>,
    /// Directory where temp files are written.
    temp_dir: PathBuf,
}

impl DiskBuffer {
    /// Create a new `DiskBuffer` using the given temp directory.
    pub fn new(temp_dir: impl Into<PathBuf>) -> Self {
        Self {
            file_queue: VecDeque::new(),
            temp_dir: temp_dir.into(),
        }
    }

    /// Create a new `DiskBuffer` using the system temp directory.
    pub fn with_system_temp() -> Self {
        Self::new(std::env::temp_dir().join("nstream_buffer"))
    }

    /// Persist `data` to a new temp file and enqueue its path.
    pub async fn push(&mut self, data: Bytes) -> Result<(), NStreamError> {
        fs::create_dir_all(&self.temp_dir)
            .await
            .map_err(|e| NStreamError::Storage(format!("Failed to create buffer dir: {}", e)))?;

        let file_name = format!("frame_{}.bin", Uuid::new_v4());
        let file_path = self.temp_dir.join(&file_name);

        fs::write(&file_path, &data).await.map_err(|e| {
            NStreamError::Storage(format!(
                "Failed to write buffer file '{}': {}",
                file_path.display(),
                e
            ))
        })?;

        debug!(path = %file_path.display(), bytes = data.len(), "Wrote frame to disk buffer");
        self.file_queue.push_back(file_path);
        Ok(())
    }

    /// Read and remove the oldest buffered frame.
    /// Returns `None` if the queue is empty.
    pub async fn pop(&mut self) -> Option<Bytes> {
        let path = self.file_queue.pop_front()?;

        match fs::read(&path).await {
            Ok(data) => {
                debug!(path = %path.display(), "Read frame from disk buffer");
                // Clean up the temp file after reading.
                if let Err(e) = fs::remove_file(&path).await {
                    error!(path = %path.display(), error = %e, "Failed to remove temp buffer file");
                }
                Some(Bytes::from(data))
            }
            Err(e) => {
                error!(path = %path.display(), error = %e, "Failed to read disk buffer frame");
                None
            }
        }
    }

    /// Returns `true` if no frames are buffered on disk.
    pub fn is_empty(&self) -> bool {
        self.file_queue.is_empty()
    }

    /// Returns the number of frames currently queued.
    pub fn len(&self) -> usize {
        self.file_queue.len()
    }

    /// Remove all queued temp files and clear the queue.
    pub async fn clear(&mut self) {
        for path in self.file_queue.drain(..) {
            if let Err(e) = fs::remove_file(&path).await {
                error!(path = %path.display(), error = %e, "Failed to remove temp file during clear");
            }
        }
        info!("Disk buffer cleared");
    }
}

use tracing::{debug, info};

use nstream_core::NStreamError;

/// Represents a single camera feed with basic metadata.
#[derive(Debug, Clone)]
pub struct CameraFeed {
    pub camera_id: u32,
    pub name: String,
    pub frame_count: u64,
    pub is_active: bool,
}

impl CameraFeed {
    pub fn new(camera_id: u32, name: impl Into<String>, frame_count: u64) -> Self {
        Self {
            camera_id,
            name: name.into(),
            frame_count,
            is_active: true,
        }
    }
}

/// Aggregates multiple camera feeds and selects the best one for processing.
pub struct MultiCameraAggregator {
    feeds: Vec<CameraFeed>,
}

impl MultiCameraAggregator {
    pub fn new() -> Self {
        Self { feeds: Vec::new() }
    }

    /// Register a camera feed.
    pub fn add_feed(&mut self, feed: CameraFeed) {
        debug!(camera_id = feed.camera_id, name = %feed.name, "Added camera feed");
        self.feeds.push(feed);
    }

    /// Remove a feed by camera_id.
    pub fn remove_feed(&mut self, camera_id: u32) {
        self.feeds.retain(|f| f.camera_id != camera_id);
        debug!(camera_id = camera_id, "Removed camera feed");
    }

    /// Select the best available camera feed index based on highest frame count.
    /// Returns `Err` if no active feeds are available.
    pub async fn select_best(&self) -> Result<usize, NStreamError> {
        let active: Vec<(usize, &CameraFeed)> = self
            .feeds
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_active)
            .collect();

        if active.is_empty() {
            return Err(NStreamError::NotFound(
                "No active camera feeds available".to_string(),
            ));
        }

        // Heuristic: pick the feed with the highest frame count.
        let (best_index, best_feed) = active
            .iter()
            .max_by_key(|(_, f)| f.frame_count)
            .copied()
            .expect("active is non-empty");

        info!(
            camera_id = best_feed.camera_id,
            name = %best_feed.name,
            frame_count = best_feed.frame_count,
            index = best_index,
            "Selected best camera feed"
        );

        Ok(best_index)
    }

    /// Update the frame count for a camera (called as frames arrive).
    pub fn update_frame_count(&mut self, camera_id: u32, frame_count: u64) {
        if let Some(feed) = self.feeds.iter_mut().find(|f| f.camera_id == camera_id) {
            feed.frame_count = frame_count;
            debug!(
                camera_id = camera_id,
                frame_count = frame_count,
                "Updated frame count"
            );
        }
    }

    /// Returns all registered feeds.
    pub fn feeds(&self) -> &[CameraFeed] {
        &self.feeds
    }
}

impl Default for MultiCameraAggregator {
    fn default() -> Self {
        Self::new()
    }
}

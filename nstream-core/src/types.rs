use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a processing job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    pub fn new() -> Self {
        JobId(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Current status of a processing job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Pending => write!(f, "pending"),
            JobStatus::Running => write!(f, "running"),
            JobStatus::Completed => write!(f, "completed"),
            JobStatus::Cancelled => write!(f, "cancelled"),
            JobStatus::Failed(msg) => write!(f, "failed: {}", msg),
        }
    }
}

/// A language pair for translation tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePair {
    pub source: String,
    pub target: String,
}

/// Metadata about an audio track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    pub track_id: u32,
    pub language: String,
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u8,
    pub duration_ms: u64,
}

/// Metadata about a video track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoTrack {
    pub track_id: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub duration_ms: u64,
    pub frame_count: u64,
}

/// A discrete scene or chunk cut from a video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneChunk {
    pub chunk_id: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub keyframe_index: u64,
    pub data: Bytes,
}

/// The central media payload that flows through the processing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPayload {
    pub job_id: JobId,
    pub raw_data: Bytes,
    pub audio_tracks: Vec<AudioTrack>,
    pub video_tracks: Vec<VideoTrack>,
    pub scene_chunks: Vec<SceneChunk>,
    pub metadata: std::collections::HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

impl MediaPayload {
    pub fn new(job_id: JobId, raw_data: Bytes) -> Self {
        Self {
            job_id,
            raw_data,
            audio_tracks: Vec::new(),
            video_tracks: Vec::new(),
            scene_chunks: Vec::new(),
            metadata: std::collections::HashMap::new(),
            created_at: Utc::now(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

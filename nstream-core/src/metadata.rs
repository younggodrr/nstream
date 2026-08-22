use crate::types::{JobId, JobStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneRange {
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Determines how the pipeline is invoked and what latency/quality tradeoffs apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineMode {
    /// High-quality offline batch processing for studio productions.
    StudioBatch,
    /// Low-latency real-time stream processing.
    LiveStream,
    /// Creator-directed selective enhancement.
    CreatorSelective,
}

impl std::fmt::Display for PipelineMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineMode::StudioBatch => write!(f, "studio_batch"),
            PipelineMode::LiveStream => write!(f, "live_stream"),
            PipelineMode::CreatorSelective => write!(f, "creator_selective"),
        }
    }
}

/// Full metadata record for a submitted job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    pub job_id: JobId,
    pub source_lang: String,
    pub target_langs: Vec<String>,
    pub status: JobStatus,
    pub pipeline_mode: PipelineMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub s3_bucket: Option<String>,
    pub s3_key: Option<String>,
    /// Delivered artifacts. In local development these are filesystem paths;
    /// production storage adapters should expose object URIs here.
    pub output_files: Vec<String>,
    pub scene_ranges: Vec<SceneRange>,
    pub error_message: Option<String>,
}

impl JobMetadata {
    pub fn new(
        job_id: JobId,
        source_lang: String,
        target_langs: Vec<String>,
        pipeline_mode: PipelineMode,
    ) -> Self {
        let now = Utc::now();
        Self {
            job_id,
            source_lang,
            target_langs,
            status: JobStatus::Pending,
            pipeline_mode,
            created_at: now,
            updated_at: now,
            s3_bucket: None,
            s3_key: None,
            output_files: Vec::new(),
            scene_ranges: Vec::new(),
            error_message: None,
        }
    }

    pub fn with_s3(mut self, bucket: String, key: String) -> Self {
        self.s3_bucket = Some(bucket);
        self.s3_key = Some(key);
        self
    }

    pub fn with_scene_ranges(mut self, scene_ranges: Vec<SceneRange>) -> Self {
        self.scene_ranges = scene_ranges;
        self
    }

    pub fn set_status(&mut self, status: JobStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.error_message = Some(message.clone());
        self.set_status(JobStatus::Failed(message));
    }
}

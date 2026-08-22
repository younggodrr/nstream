use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use nstream_core::{JobId, JobStatus, PipelineMode, SceneRange};

/// Request body for submitting a new job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitJobRequest {
    /// BCP-47 language code of the source content (e.g. "en", "es").
    pub source_lang: String,
    /// List of target BCP-47 language codes for translation/dubbing.
    pub target_langs: Vec<String>,
    /// Processing mode that governs quality/latency tradeoffs.
    pub pipeline_mode: PipelineMode,
    /// Optional S3 bucket containing the source media.
    pub s3_bucket: Option<String>,
    /// Optional S3 object key of the source media.
    pub s3_key: Option<String>,
    pub scene_ranges: Option<Vec<SceneRange>>,
}

/// Response returned after submitting or querying a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResponse {
    pub job_id: JobId,
    pub status: JobStatus,
    pub source_lang: String,
    pub target_langs: Vec<String>,
    pub pipeline_mode: PipelineMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub s3_bucket: Option<String>,
    pub s3_key: Option<String>,
    pub output_files: Vec<String>,
    pub error_message: Option<String>,
    pub scene_ranges: Vec<SceneRange>,
}

/// Request body for registering a webhook URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterWebhookRequest {
    /// The HTTPS URL to receive job-status notifications.
    pub url: String,
    /// Optional list of event types to subscribe to (e.g. ["completed", "failed"]).
    pub events: Option<Vec<String>>,
}

/// Response after successfully registering a webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterWebhookResponse {
    pub id: String,
    pub url: String,
    pub signing_secret: String,
    pub registered_at: DateTime<Utc>,
}

/// Generic API error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactAccessResponse {
    pub url: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptSegmentResponse {
    pub start_ms: u64,
    pub end_ms: u64,
    pub language: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobStageResponse {
    pub stage: String,
    pub status: String,
    pub detail: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobEventResponse {
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateUploadRequest {
    pub file_name: String,
    pub content_type: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct CreateUploadResponse {
    pub bucket: String,
    pub key: String,
    pub upload_url: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartLiveSessionRequest {
    pub source_url: String,
    pub publish_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveSessionResponse {
    pub session_id: String,
    pub status: String,
    pub playlist_path: String,
    pub publish_status: Option<String>,
}

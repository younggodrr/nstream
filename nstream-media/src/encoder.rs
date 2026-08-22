use async_trait::async_trait;
use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

use crate::formats::OutputFormat;
use nstream_core::{MediaPayload, NStreamError};

/// Trait for encoding a `MediaPayload` to a media file.
#[async_trait]
pub trait MediaEncoder: Send + Sync {
    async fn encode(
        &self,
        payload: MediaPayload,
        output_path: &Path,
        format: OutputFormat,
    ) -> Result<(), NStreamError>;
}

/// FFmpeg-backed encoder. Writes the input payload to a temporary file and transcodes it.
pub struct FfmpegEncoder;

impl FfmpegEncoder {
    pub fn new() -> Self {
        Self
    }

    /// Build the ffmpeg command arguments for the given format and paths.
    fn build_args(input_path: &Path, output_path: &Path, format: &OutputFormat) -> Vec<String> {
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input_path.display().to_string(),
            "-c:v".to_string(),
            format.ffmpeg_video_codec().to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-f".to_string(),
            format.ffmpeg_format().to_string(),
            output_path.display().to_string(),
        ]
    }
}

#[async_trait]
impl MediaEncoder for FfmpegEncoder {
    async fn encode(
        &self,
        payload: MediaPayload,
        output_path: &Path,
        format: OutputFormat,
    ) -> Result<(), NStreamError> {
        info!(
            job_id = %payload.job_id,
            output = %output_path.display(),
            format = %format,
            "FfmpegEncoder: encoding"
        );

        // Write raw payload bytes to a temp input file.
        let temp_input =
            std::env::temp_dir().join(format!("nstream_encode_input_{}.bin", payload.job_id));

        tokio::fs::write(&temp_input, &payload.raw_data).await?;

        let args = Self::build_args(&temp_input, output_path, &format);
        info!(
            job_id = %payload.job_id,
            command = %format!("ffmpeg {}", args.join(" ")),
            "FfmpegEncoder: would run"
        );

        // Never report a successful artifact if encoding did not actually succeed.
        let result = Command::new("ffmpeg").args(&args).output();
        match result {
            Ok(output) if output.status.success() => {
                info!(
                    job_id = %payload.job_id,
                    output = %output_path.display(),
                    "FfmpegEncoder: encode complete"
                );
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return Err(NStreamError::Decode(format!(
                    "ffmpeg encode failed: {stderr}"
                )));
            }
            Err(e) => {
                return Err(NStreamError::Decode(format!("ffmpeg could not start: {e}")));
            }
        }

        // Clean up temp input file.
        if let Err(e) = tokio::fs::remove_file(&temp_input).await {
            warn!(path = %temp_input.display(), error = %e, "Failed to remove temp input file");
        }

        Ok(())
    }
}

impl Default for FfmpegEncoder {
    fn default() -> Self {
        Self::new()
    }
}

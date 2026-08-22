use async_trait::async_trait;
use bytes::Bytes;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info};

use nstream_core::{AudioTrack, JobId, MediaPayload, NStreamError, VideoTrack};

/// Trait for decoding a media file into a `MediaPayload`.
#[async_trait]
pub trait MediaDecoder: Send + Sync {
    async fn decode(&self, path: &Path) -> Result<MediaPayload, NStreamError>;
}

/// FFmpeg-backed decoder. It validates the container with ffprobe and extracts
/// authoritative audio/video stream metadata for downstream policy and QC.
pub struct FfmpegDecoder;

impl FfmpegDecoder {
    pub fn new() -> Self {
        Self
    }

    /// Run ffprobe on the file and return its stdout as a String.
    fn probe(&self, path: &Path) -> Result<String, NStreamError> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
                path.to_str().unwrap_or(""),
            ])
            .output()
            .map_err(|e| NStreamError::Decode(format!("ffprobe execution failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(NStreamError::Decode(format!(
                "ffprobe exited with status {}: {}",
                output.status, stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl MediaDecoder for FfmpegDecoder {
    async fn decode(&self, path: &Path) -> Result<MediaPayload, NStreamError> {
        info!(path = %path.display(), "FfmpegDecoder: decoding");

        let raw_bytes = tokio::fs::read(path).await.map_err(|e| {
            NStreamError::Decode(format!("Failed to read '{}': {}", path.display(), e))
        })?;

        let job_id = JobId::new();
        let mut payload = MediaPayload::new(job_id, Bytes::from(raw_bytes));
        payload
            .metadata
            .insert("source_path".to_string(), path.display().to_string());

        let probe_json = self.probe(path)?;
        let probe: Probe = serde_json::from_str(&probe_json)
            .map_err(|error| NStreamError::Decode(format!("invalid ffprobe response: {error}")))?;
        let fallback_duration = probe
            .format
            .and_then(|format| format.duration)
            .map(seconds_to_ms)
            .unwrap_or(0);
        for stream in probe.streams {
            match stream.codec_type.as_deref() {
                Some("video") => payload.video_tracks.push(VideoTrack {
                    track_id: stream.index.unwrap_or_default(),
                    codec: stream.codec_name.unwrap_or_else(|| "unknown".into()),
                    width: stream.width.unwrap_or_default(),
                    height: stream.height.unwrap_or_default(),
                    frame_rate: stream
                        .avg_frame_rate
                        .as_deref()
                        .map(frame_rate)
                        .unwrap_or(0.0),
                    duration_ms: stream
                        .duration
                        .map(seconds_to_ms)
                        .unwrap_or(fallback_duration),
                    frame_count: stream.nb_frames.and_then(|v| v.parse().ok()).unwrap_or(0),
                }),
                Some("audio") => payload.audio_tracks.push(AudioTrack {
                    track_id: stream.index.unwrap_or_default(),
                    language: stream
                        .tags
                        .and_then(|tags| tags.language)
                        .unwrap_or_else(|| "und".into()),
                    codec: stream.codec_name.unwrap_or_else(|| "unknown".into()),
                    sample_rate: stream.sample_rate.and_then(|v| v.parse().ok()).unwrap_or(0),
                    channels: stream.channels.unwrap_or_default() as u8,
                    duration_ms: stream
                        .duration
                        .map(seconds_to_ms)
                        .unwrap_or(fallback_duration),
                }),
                _ => {}
            }
        }
        debug!(job_id = %job_id, video_tracks = payload.video_tracks.len(), audio_tracks = payload.audio_tracks.len(), "ffprobe metadata parsed");

        info!(
            job_id = %job_id,
            path = %path.display(),
            bytes = payload.raw_data.len(),
            "FfmpegDecoder: decode complete"
        );
        Ok(payload)
    }
}

#[derive(Deserialize)]
struct Probe {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}
#[derive(Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
}
#[derive(Deserialize)]
struct ProbeStream {
    index: Option<u32>,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    duration: Option<String>,
    nb_frames: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    tags: Option<ProbeTags>,
}
#[derive(Deserialize)]
struct ProbeTags {
    language: Option<String>,
}
fn seconds_to_ms(value: String) -> u64 {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| (v * 1000.0).round() as u64)
        .unwrap_or(0)
}
fn frame_rate(value: &str) -> f64 {
    match value.split_once('/') {
        Some((a, b)) => match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(a), Ok(b)) if b > 0.0 => a / b,
            _ => 0.0,
        },
        None => value.parse().unwrap_or(0.0),
    }
}

impl Default for FfmpegDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{frame_rate, seconds_to_ms};

    #[test]
    fn parses_rational_frame_rates() {
        assert!((frame_rate("30000/1001") - 29.970029).abs() < 0.0001);
        assert_eq!(frame_rate("0/0"), 0.0);
    }

    #[test]
    fn rejects_invalid_durations() {
        assert_eq!(seconds_to_ms("12.345".into()), 12_345);
        assert_eq!(seconds_to_ms("-1".into()), 0);
        assert_eq!(seconds_to_ms("not-a-number".into()), 0);
    }
}

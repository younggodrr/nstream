use std::path::Path;
use tokio::process::Command;

use nstream_core::NStreamError;

/// A bounded PCM chunk ready for ASR. Samples are 16 kHz, mono, IEEE f32.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub start_ms: u64,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

/// Extract a bounded audio interval for ASR using FFmpeg.
///
/// Callers should request short sequential chunks (for example 30 seconds) rather
/// than materialising an entire feature-length soundtrack in memory.
pub async fn extract_asr_audio_chunk(
    source: &Path,
    start_ms: u64,
    duration_ms: u64,
) -> Result<AudioChunk, NStreamError> {
    if duration_ms == 0 || duration_ms > 60_000 {
        return Err(NStreamError::Decode(
            "ASR audio chunk duration must be between 1 and 60,000 ms".to_string(),
        ));
    }
    let start_seconds = format!("{:.3}", start_ms as f64 / 1000.0);
    let duration_seconds = format!("{:.3}", duration_ms as f64 / 1000.0);
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-ss",
            &start_seconds,
            "-t",
            &duration_seconds,
            "-i",
            source
                .to_str()
                .ok_or_else(|| NStreamError::Decode("source path is not UTF-8".to_string()))?,
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-f",
            "f32le",
            "pipe:1",
        ])
        .output()
        .await
        .map_err(|error| NStreamError::Decode(format!("could not start ffmpeg: {error}")))?;
    if !output.status.success() {
        return Err(NStreamError::Decode(format!(
            "ffmpeg audio extraction failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if output.stdout.len() % 4 != 0 {
        return Err(NStreamError::Decode(
            "ffmpeg returned malformed f32 PCM".to_string(),
        ));
    }
    let samples = output
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("chunks are exactly four bytes")))
        .collect();
    Ok(AudioChunk {
        start_ms,
        sample_rate: 16_000,
        samples,
    })
}

/// Reads duration using ffprobe before splitting a source into bounded ASR chunks.
pub async fn probe_duration_ms(source: &Path) -> Result<u64, NStreamError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nokey=1:noprint_wrappers=1",
            source
                .to_str()
                .ok_or_else(|| NStreamError::Decode("source path is not UTF-8".to_string()))?,
        ])
        .output()
        .await
        .map_err(|e| NStreamError::Decode(format!("could not start ffprobe: {e}")))?;
    if !output.status.success() {
        return Err(NStreamError::Decode(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let seconds = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .map_err(|_| NStreamError::Decode("ffprobe returned an invalid duration".to_string()))?;
    Ok((seconds * 1000.0).ceil() as u64)
}

/// Rejects a rendered artifact unless ffprobe can read both a video and an
/// audio stream and reports a positive duration. This is the final media QC
/// gate before an artifact is exposed to a tenant.
pub async fn validate_rendered_mp4(output: &Path) -> Result<(), NStreamError> {
    let duration = probe_duration_ms(output).await?;
    if duration == 0 {
        return Err(NStreamError::Decode(
            "rendered output has zero duration".into(),
        ));
    }
    let result = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
            output
                .to_str()
                .ok_or_else(|| NStreamError::Decode("output path is not UTF-8".into()))?,
        ])
        .output()
        .await
        .map_err(|e| NStreamError::Decode(format!("could not start ffprobe: {e}")))?;
    if !result.status.success() {
        return Err(NStreamError::Decode(format!(
            "ffprobe output validation failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    let streams = String::from_utf8_lossy(&result.stdout);
    if !streams.lines().any(|line| line.trim() == "video")
        || !streams.lines().any(|line| line.trim() == "audio")
    {
        return Err(NStreamError::Decode(
            "rendered output must contain both video and audio streams".into(),
        ));
    }
    Ok(())
}

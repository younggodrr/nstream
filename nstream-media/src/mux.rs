use std::path::Path;
use std::process::Command;
use tracing::info;

use nstream_core::NStreamError;

/// Muxes separate video and audio tracks into a single output file using FFmpeg.
pub struct Muxer;

impl Muxer {
    pub fn new() -> Self {
        Self
    }

    /// Mux a video file and an audio file into a single output using stream copy.
    /// Runs: `ffmpeg -i <video> -i <audio> -c copy <output>`
    pub async fn mux_tracks(
        &self,
        video: &Path,
        audio: &Path,
        output: &Path,
    ) -> Result<(), NStreamError> {
        info!(
            video = %video.display(),
            audio = %audio.display(),
            output = %output.display(),
            "Muxer: muxing tracks"
        );

        let args = [
            "-y",
            "-i",
            video.to_str().unwrap_or(""),
            "-i",
            audio.to_str().unwrap_or(""),
            "-c",
            "copy",
            output.to_str().unwrap_or(""),
        ];

        info!(
            command = %format!("ffmpeg {}", args.join(" ")),
            "Muxer: running command"
        );

        let result = Command::new("ffmpeg").args(&args).output();

        match result {
            Ok(out) if out.status.success() => {
                info!(output = %output.display(), "Muxer: mux complete");
                Ok(())
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                Err(NStreamError::Decode(format!("ffmpeg mux failed: {stderr}")))
            }
            Err(e) => Err(NStreamError::Decode(format!("ffmpeg could not start: {e}"))),
        }
    }
}

impl Default for Muxer {
    fn default() -> Self {
        Self::new()
    }
}

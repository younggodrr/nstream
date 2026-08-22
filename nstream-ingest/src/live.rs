use std::path::{Path, PathBuf};

use tokio::process::{Child, Command};
use uuid::Uuid;

use nstream_core::NStreamError;

/// A supervised FFmpeg ingest process. FFmpeg handles protocol-compliant RTMP
/// and SRT parsing; the Rust process owns lifecycle, path validation, and the
/// rolling HLS window used by the live processing path.
pub struct LiveIngestSession {
    pub id: Uuid,
    pub source_url: String,
    pub playlist: PathBuf,
    child: Child,
}

/// Republishes a rolling HLS playlist to an RTMP/RTMPS/SRT destination. This
/// keeps transport concerns in FFmpeg while Rust owns tenant/session lifecycle.
pub struct LivePublishSession {
    pub target_url: String,
    child: Child,
}

impl LiveIngestSession {
    pub async fn start(
        source_url: impl Into<String>,
        output_root: &Path,
    ) -> Result<Self, NStreamError> {
        let source_url = source_url.into();
        let parsed = reqwest::Url::parse(&source_url).map_err(|_| {
            NStreamError::Decode("live source must be an absolute RTMP, RTMPS, or SRT URL".into())
        })?;
        if !matches!(parsed.scheme(), "rtmp" | "rtmps" | "srt") {
            return Err(NStreamError::Decode(
                "live source scheme must be rtmp, rtmps, or srt".into(),
            ));
        }
        let id = Uuid::new_v4();
        let directory = output_root.join(id.to_string());
        tokio::fs::create_dir_all(&directory).await?;
        let playlist = directory.join("index.m3u8");
        let child = Command::new("ffmpeg")
            .args(["-hide_banner", "-nostdin", "-i", &source_url, "-c", "copy"])
            .args(["-f", "hls", "-hls_time", "2", "-hls_list_size", "150"])
            .args([
                "-hls_flags",
                "delete_segments+append_list+program_date_time",
            ])
            .arg(&playlist)
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| NStreamError::Io(error))?;
        Ok(Self {
            id,
            source_url,
            playlist,
            child,
        })
    }

    pub async fn stop(&mut self) -> Result<(), NStreamError> {
        self.child.kill().await.map_err(NStreamError::Io)
    }

    pub async fn is_running(&mut self) -> Result<bool, NStreamError> {
        Ok(self.child.try_wait()?.is_none())
    }
}

impl LivePublishSession {
    pub async fn start(
        playlist: &Path,
        target_url: impl Into<String>,
    ) -> Result<Self, NStreamError> {
        let target_url = target_url.into();
        let parsed = reqwest::Url::parse(&target_url).map_err(|_| {
            NStreamError::Decode(
                "publish target must be an absolute RTMP, RTMPS, or SRT URL".into(),
            )
        })?;
        let format = match parsed.scheme() {
            "rtmp" | "rtmps" => "flv",
            "srt" => "mpegts",
            _ => {
                return Err(NStreamError::Decode(
                    "publish target scheme must be rtmp, rtmps, or srt".into(),
                ))
            }
        };
        let child = Command::new("ffmpeg")
            .args(["-hide_banner", "-nostdin", "-re", "-i"])
            .arg(playlist)
            .args(["-c", "copy", "-f", format])
            .arg(&target_url)
            .kill_on_drop(true)
            .spawn()
            .map_err(NStreamError::Io)?;
        Ok(Self { target_url, child })
    }

    pub async fn stop(&mut self) -> Result<(), NStreamError> {
        self.child.kill().await.map_err(NStreamError::Io)
    }
    pub async fn is_running(&mut self) -> Result<bool, NStreamError> {
        Ok(self.child.try_wait()?.is_none())
    }
}

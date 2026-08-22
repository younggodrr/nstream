use serde::{Deserialize, Serialize};

/// Supported output container/codec formats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Apple ProRes — high-quality mastering format.
    ProRes,
    /// Avid DNxHR — broadcast-quality mastering format.
    DnxHr,
    /// H.264/AAC in an MPEG-4 container — universal delivery.
    Mp4,
    /// VP9/Opus in a WebM container — web streaming.
    Webm,
}

impl OutputFormat {
    /// Returns the typical file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::ProRes => "mov",
            OutputFormat::DnxHr => "mxf",
            OutputFormat::Mp4 => "mp4",
            OutputFormat::Webm => "webm",
        }
    }

    /// Returns the FFmpeg format string for `-f` flag.
    pub fn ffmpeg_format(&self) -> &'static str {
        match self {
            OutputFormat::ProRes => "mov",
            OutputFormat::DnxHr => "mxf",
            OutputFormat::Mp4 => "mp4",
            OutputFormat::Webm => "webm",
        }
    }

    /// Returns the FFmpeg video codec for this format.
    pub fn ffmpeg_video_codec(&self) -> &'static str {
        match self {
            OutputFormat::ProRes => "prores_ks",
            OutputFormat::DnxHr => "dnxhd",
            OutputFormat::Mp4 => "libx264",
            OutputFormat::Webm => "libvpx-vp9",
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::ProRes => write!(f, "prores"),
            OutputFormat::DnxHr => write!(f, "dnxhr"),
            OutputFormat::Mp4 => write!(f, "mp4"),
            OutputFormat::Webm => write!(f, "webm"),
        }
    }
}

/// Information about a scene chunk identified in a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneChunkInfo {
    pub chunk_index: u32,
    pub start_timecode: String,
    pub end_timecode: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub duration_ms: u64,
    pub keyframe_pts: u64,
}

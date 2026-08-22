use nstream_core::MediaPayload;

/// All inference task types that can be submitted to the worker pool.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum InferenceTask {
    /// Automatic Speech Recognition — transcribe audio from a media payload.
    Asr(MediaPayload),
    /// Text-to-Speech — synthesize speech from text in a given language.
    Tts { text: String, lang: String },
    /// Lip-sync — re-time mouth movements to match new audio.
    LipSync(MediaPayload),
    /// Upscale — increase video resolution via AI super-resolution.
    Upscale(MediaPayload),
    /// VFX Detection — detect and segment visual effects regions.
    VfxDetect(MediaPayload),
}

impl InferenceTask {
    /// Returns a short string label for tracing/logging.
    pub fn task_type(&self) -> &'static str {
        match self {
            InferenceTask::Asr(_) => "asr",
            InferenceTask::Tts { .. } => "tts",
            InferenceTask::LipSync(_) => "lip_sync",
            InferenceTask::Upscale(_) => "upscale",
            InferenceTask::VfxDetect(_) => "vfx_detect",
        }
    }
}

/// All inference result types, mirroring `InferenceTask`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum InferenceResult {
    /// Transcribed text from ASR.
    Asr(String),
    /// Raw audio bytes from TTS synthesis.
    Tts(Vec<u8>),
    /// Updated media payload with lip-sync applied.
    LipSync(MediaPayload),
    /// Updated media payload with upscaling applied.
    Upscale(MediaPayload),
    /// Updated media payload with VFX regions annotated.
    VfxDetect(MediaPayload),
}

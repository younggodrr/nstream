use crate::tasks::{InferenceResult, InferenceTask};
use nstream_core::{MediaPayload, NStreamError};
use std::path::PathBuf;
use tracing::debug;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Local GGML Whisper runner. Input must be 16 kHz mono f32 PCM.
pub struct WhisperAsr {
    model_path: PathBuf,
}

impl WhisperAsr {
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
        }
    }

    pub async fn transcribe_pcm(
        &self,
        samples: Vec<f32>,
        language: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, NStreamError> {
        let model_path = self.model_path.clone();
        tokio::task::spawn_blocking(move || {
            let ctx =
                WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
                    .map_err(|e| {
                    NStreamError::Inference(format!(
                        "could not load Whisper model '{}': {e}",
                        model_path.display()
                    ))
                })?;
            let mut state = ctx.create_state().map_err(|e| {
                NStreamError::Inference(format!("could not create Whisper state: {e}"))
            })?;
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(language.as_deref());
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            state.full(params, &samples).map_err(|e| {
                NStreamError::Inference(format!("Whisper transcription failed: {e}"))
            })?;
            Ok(state
                .as_iter()
                .map(|segment| TranscriptSegment {
                    start_ms: segment.start_timestamp().max(0) as u64 * 10,
                    end_ms: segment.end_timestamp().max(0) as u64 * 10,
                    text: segment.to_string().trim().to_string(),
                })
                .filter(|segment| !segment.text.is_empty())
                .collect())
        })
        .await
        .map_err(|e| NStreamError::Join(e.to_string()))?
    }
}

/// A worker boundary for licensed, model-backed inference. It must never fabricate media output.
#[allow(dead_code)]
pub struct InferenceWorker {
    pub worker_id: u32,
    pub model_path: PathBuf,
}

impl InferenceWorker {
    pub fn new(worker_id: u32, model_path: impl Into<PathBuf>) -> Self {
        Self {
            worker_id,
            model_path: model_path.into(),
        }
    }
    pub async fn run_asr(&self, payload: MediaPayload) -> Result<String, NStreamError> {
        debug!(worker_id = self.worker_id, job_id = %payload.job_id, "ASR requested");
        Err(unconfigured("ASR", &self.model_path))
    }

    /// Transcribe a chunk returned by `nstream_media::extract_asr_audio_chunk`.
    pub async fn run_asr_pcm(
        &self,
        samples: Vec<f32>,
        language: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, NStreamError> {
        WhisperAsr::new(&self.model_path)
            .transcribe_pcm(samples, language)
            .await
    }
    pub async fn run_tts(&self, text: String, lang: String) -> Result<Vec<u8>, NStreamError> {
        debug!(worker_id = self.worker_id, %lang, text_len = text.len(), "TTS requested");
        Err(unconfigured("TTS", &self.model_path))
    }
    pub async fn run_lipsync(&self, payload: MediaPayload) -> Result<MediaPayload, NStreamError> {
        debug!(worker_id = self.worker_id, job_id = %payload.job_id, "lip sync requested");
        Err(unconfigured("lip-sync", &self.model_path))
    }
    pub async fn run_upscale(&self, payload: MediaPayload) -> Result<MediaPayload, NStreamError> {
        debug!(worker_id = self.worker_id, job_id = %payload.job_id, "upscale requested");
        Err(unconfigured("upscale", &self.model_path))
    }
    pub async fn run_vfx_detect(
        &self,
        payload: MediaPayload,
    ) -> Result<MediaPayload, NStreamError> {
        debug!(worker_id = self.worker_id, job_id = %payload.job_id, "VFX requested");
        Err(unconfigured("VFX detection", &self.model_path))
    }
    pub async fn run_task(&self, task: InferenceTask) -> Result<InferenceResult, NStreamError> {
        match task {
            InferenceTask::Asr(payload) => Ok(InferenceResult::Asr(self.run_asr(payload).await?)),
            InferenceTask::Tts { text, lang } => {
                Ok(InferenceResult::Tts(self.run_tts(text, lang).await?))
            }
            InferenceTask::LipSync(payload) => {
                Ok(InferenceResult::LipSync(self.run_lipsync(payload).await?))
            }
            InferenceTask::Upscale(payload) => {
                Ok(InferenceResult::Upscale(self.run_upscale(payload).await?))
            }
            InferenceTask::VfxDetect(payload) => Ok(InferenceResult::VfxDetect(
                self.run_vfx_detect(payload).await?,
            )),
        }
    }
}

fn unconfigured(operation: &str, path: &std::path::Path) -> NStreamError {
    NStreamError::Inference(format!("{operation} model is not configured at '{}'; install a licensed model provider before enabling this stage", path.display()))
}

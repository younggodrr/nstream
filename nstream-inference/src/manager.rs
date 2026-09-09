use crate::full_pipeline::FullPipelineClient;
use crate::remote::RemoteWhisperAsr;
use crate::worker::{TranscriptSegment, WhisperAsr};
use nstream_core::NStreamError;
use std::path::PathBuf;

/// Inference mode - local or remote
#[derive(Debug, Clone)]
pub enum InferenceMode {
    Local { model_path: PathBuf },
    Remote { base_url: String, api_key: Option<String> },
    FullPipeline { base_url: String, api_key: Option<String> },
}

/// Unified inference interface that can use either local or remote inference
pub struct InferenceManager {
    mode: InferenceMode,
}

impl InferenceManager {
    pub fn new(mode: InferenceMode) -> Self {
        Self { mode }
    }

    /// Create from environment variables
    /// Uses FULL_PIPELINE_URL if set, otherwise REMOTE_INFERENCE_URL, otherwise falls back to local
    pub fn from_env() -> Result<Self, NStreamError> {
        if let Ok(base_url) = std::env::var("FULL_PIPELINE_URL") {
            let api_key = std::env::var("FULL_PIPELINE_API_KEY").ok();
            Ok(Self::new(InferenceMode::FullPipeline { base_url, api_key }))
        } else if let Ok(base_url) = std::env::var("REMOTE_INFERENCE_URL") {
            let api_key = std::env::var("REMOTE_INFERENCE_API_KEY").ok();
            Ok(Self::new(InferenceMode::Remote { base_url, api_key }))
        } else if let Ok(model_path) = std::env::var("WHISPER_MODEL_PATH") {
            Ok(Self::new(InferenceMode::Local {
                model_path: PathBuf::from(model_path),
            }))
        } else {
            Err(NStreamError::Inference(
                "Either FULL_PIPELINE_URL, REMOTE_INFERENCE_URL, or WHISPER_MODEL_PATH must be set".to_string(),
            ))
        }
    }

    /// Transcribe audio data using the configured inference mode
    pub async fn transcribe_pcm(
        &self,
        samples: Vec<f32>,
        language: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, NStreamError> {
        match &self.mode {
            InferenceMode::Local { model_path } => {
                let asr = WhisperAsr::new(model_path);
                asr.transcribe_pcm(samples, language).await
            }
            InferenceMode::Remote { base_url, api_key } => {
                // Convert PCM samples to WAV format
                let wav_data = self.pcm_to_wav(&samples)?;
                
                let remote = RemoteWhisperAsr::new(base_url, api_key.clone());
                remote.transcribe_audio(wav_data, language).await
            }
            InferenceMode::FullPipeline { base_url, api_key } => {
                // Convert PCM samples to WAV format
                let wav_data = self.pcm_to_wav(&samples)?;
                
                let remote = RemoteWhisperAsr::new(base_url, api_key.clone());
                remote.transcribe_audio(wav_data, language).await
            }
        }
    }

    /// Check health of the inference backend
    pub async fn health_check(&self) -> Result<String, NStreamError> {
        match &self.mode {
            InferenceMode::Local { model_path } => {
                Ok(format!("Local inference with model: {}", model_path.display()))
            }
            InferenceMode::Remote { base_url, api_key } => {
                let remote = RemoteWhisperAsr::new(base_url, api_key.clone());
                let health = remote.health_check().await?;
                Ok(format!(
                    "Remote inference - GPU: {}, Device: {}, Model loaded: {}",
                    health.gpu_available, health.device, health.model_loaded
                ))
            }
            InferenceMode::FullPipeline { base_url, api_key } => {
                let full_pipeline = FullPipelineClient::new(base_url, api_key.clone());
                let health = full_pipeline.health_check().await?;
                Ok(format!(
                    "Full pipeline - GPU: {}, Device: {}, Model loaded: {}, Capabilities: ASR={}, TTS={}, Translation={}, LipSync={}",
                    health.gpu_available, health.device, health.model_loaded,
                    health.capabilities.asr, health.capabilities.tts, 
                    health.capabilities.translation, health.capabilities.lip_sync
                ))
            }
        }
    }

    /// Generate speech from text (requires FullPipeline mode)
    pub async fn text_to_speech(
        &self,
        text: &str,
        language: &str,
    ) -> Result<Vec<u8>, NStreamError> {
        match &self.mode {
            InferenceMode::FullPipeline { base_url, api_key } => {
                let full_pipeline = FullPipelineClient::new(base_url, api_key.clone());
                full_pipeline.text_to_speech(text, language).await
            }
            _ => Err(NStreamError::Inference(
                "Text-to-speech requires FULL_PIPELINE_URL to be configured".to_string(),
            )),
        }
    }

    /// Translate text (requires FullPipeline mode)
    pub async fn translate_text(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, NStreamError> {
        match &self.mode {
            InferenceMode::FullPipeline { base_url, api_key } => {
                let full_pipeline = FullPipelineClient::new(base_url, api_key.clone());
                full_pipeline.translate_text(text, source_lang, target_lang).await
            }
            _ => Err(NStreamError::Inference(
                "Translation requires FULL_PIPELINE_URL to be configured".to_string(),
            )),
        }
    }

    /// Process lip-sync (requires FullPipeline mode)
    pub async fn lip_sync_video(
        &self,
        video_data: Vec<u8>,
        target_language: &str,
    ) -> Result<Vec<u8>, NStreamError> {
        match &self.mode {
            InferenceMode::FullPipeline { base_url, api_key } => {
                let full_pipeline = FullPipelineClient::new(base_url, api_key.clone());
                full_pipeline.lip_sync_video(video_data, target_language).await
            }
            _ => Err(NStreamError::Inference(
                "Lip-sync requires FULL_PIPELINE_URL to be configured".to_string(),
            )),
        }
    }

    /// Convert PCM samples to WAV format for remote transmission
    fn pcm_to_wav(&self, samples: &[f32]) -> Result<Vec<u8>, NStreamError> {
        // Simple WAV header + PCM data
        // Assumes 16kHz mono PCM (standard for Whisper)
        let sample_rate = 16000u32;
        let num_channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
        let block_align = num_channels * bits_per_sample / 8;
        let data_size = samples.len() * 2; // 16-bit = 2 bytes per sample
        let file_size = 36 + data_size as u32;

        let mut wav = Vec::with_capacity(44 + data_size);

        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");

        // fmt chunk
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        wav.extend_from_slice(&num_channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data chunk
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data_size as u32).to_le_bytes());

        // Convert f32 samples to i16
        for &sample in samples {
            let i16_sample = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            wav.extend_from_slice(&i16_sample.to_le_bytes());
        }

        Ok(wav)
    }
}
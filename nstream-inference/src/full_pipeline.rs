use nstream_core::NStreamError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Full pipeline remote client for complete dubbing workflow
pub struct FullPipelineClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl FullPipelineClient {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.into(),
            api_key,
        }
    }

    /// Check health of the full pipeline worker
    pub async fn health_check(&self) -> Result<FullPipelineHealth, NStreamError> {
        let mut request = self.client.get(&format!("{}/health", self.base_url));
        
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to connect to pipeline worker: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(NStreamError::Inference(format!(
                "Pipeline worker health check failed: {}",
                response.status()
            )));
        }

        let health: FullPipelineHealth = response.json().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to parse health response: {}", e))
        })?;

        Ok(health)
    }

    /// Generate speech from text using remote TTS
    pub async fn text_to_speech(
        &self,
        text: &str,
        language: &str,
    ) -> Result<Vec<u8>, NStreamError> {
        let mut request = self.client.post(&format!("{}/tts", self.base_url));

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let tts_request = TTSRequest {
            text: text.to_string(),
            language: language.to_string(),
            output_format: "mp3".to_string(),
        };

        let response = request.json(&tts_request).send().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to send TTS request: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NStreamError::Inference(format!(
                "TTS failed: {} - {}",
                status,
                error_text
            )));
        }

        let audio_bytes = response.bytes().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to get TTS audio: {}", e))
        })?;

        Ok(audio_bytes.to_vec())
    }

    /// Translate text using remote translation service
    pub async fn translate_text(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, NStreamError> {
        let mut request = self.client.post(&format!("{}/translate", self.base_url));

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let translation_request = TranslationRequest {
            text: text.to_string(),
            source_lang: source_lang.to_string(),
            target_lang: target_lang.to_string(),
        };

        let response = request.json(&translation_request).send().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to send translation request: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NStreamError::Inference(format!(
                "Translation failed: {} - {}",
                status,
                error_text
            )));
        }

        let result: TranslationResponse = response.json().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to parse translation response: {}", e))
        })?;

        Ok(result.translated_text)
    }

    /// Process lip-sync using remote service
    pub async fn lip_sync_video(
        &self,
        video_data: Vec<u8>,
        target_language: &str,
    ) -> Result<Vec<u8>, NStreamError> {
        let mut request = self.client.post(&format!("{}/lipsync", self.base_url));

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        request = request.header("X-NStream-Target-Language", target_language);

        // Create multipart form data
        let part = reqwest::multipart::Part::bytes(video_data)
            .file_name("video.mp4")
            .mime_str("video/mp4")
            .map_err(|e| NStreamError::Inference(format!("Failed to create multipart: {}", e)))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let response = request.multipart(form).send().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to send video to lip-sync service: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NStreamError::Inference(format!(
                "Lip-sync failed: {} - {}",
                status,
                error_text
            )));
        }

        let video_bytes = response.bytes().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to get lip-synced video: {}", e))
        })?;

        Ok(video_bytes.to_vec())
    }
}

#[derive(Debug, Deserialize)]
pub struct FullPipelineHealth {
    pub status: String,
    pub gpu_available: bool,
    pub model_loaded: bool,
    pub device: String,
    pub capabilities: PipelineCapabilities,
}

#[derive(Debug, Deserialize)]
pub struct PipelineCapabilities {
    pub asr: bool,
    pub tts: bool,
    pub translation: bool,
    pub lip_sync: bool,
}

#[derive(Debug, Serialize)]
struct TTSRequest {
    text: String,
    language: String,
    output_format: String,
}

#[derive(Debug, Serialize)]
struct TranslationRequest {
    text: String,
    source_lang: String,
    target_lang: String,
}

#[derive(Debug, Deserialize)]
struct TranslationResponse {
    translated_text: String,
    #[allow(dead_code)]
    source_lang: String,
    #[allow(dead_code)]
    target_lang: String,
}
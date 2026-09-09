use crate::worker::TranscriptSegment;
use nstream_core::NStreamError;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;



#[derive(Debug, Deserialize)]
struct TranscribeSegment {
    start_ms: f64,
    end_ms: f64,
    text: String,
}

#[derive(Debug, Deserialize)]
struct TranscribeResponse {
    text: String,
    #[allow(dead_code)]
    language: String,
    duration: f64,
    segments: Vec<TranscribeSegment>,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub gpu_available: bool,
    pub model_loaded: bool,
    pub device: String,
}

/// Remote Whisper client that calls the Colab GPU worker via HTTP
pub struct RemoteWhisperAsr {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl RemoteWhisperAsr {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout for transcription
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.into(),
            api_key,
        }
    }

    /// Check if the remote worker is healthy and has GPU available
    pub async fn health_check(&self) -> Result<HealthResponse, NStreamError> {
        let mut request = self.client.get(&format!("{}/health", self.base_url));
        
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to connect to remote worker: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(NStreamError::Inference(format!(
                "Remote worker health check failed: {}",
                response.status()
            )));
        }

        let health: HealthResponse = response.json().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to parse health response: {}", e))
        })?;

        Ok(health)
    }

    /// Transcribe audio by sending it to the remote worker
    /// Audio data should be WAV format bytes
    pub async fn transcribe_audio(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, NStreamError> {
        let mut request = self.client.post(&format!("{}/transcribe", self.base_url));

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        if let Some(lang) = &language {
            request = request.header("X-NStream-Language", lang);
        }

        // Create multipart form data
        let part = reqwest::multipart::Part::bytes(audio_data)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| NStreamError::Inference(format!("Failed to create multipart: {}", e)))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let response = request.multipart(form).send().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to send audio to remote worker: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NStreamError::Inference(format!(
                "Remote transcription failed: {} - {}",
                status,
                error_text
            )));
        }

        let result: TranscribeResponse = response.json().await.map_err(|e| {
            NStreamError::Inference(format!("Failed to parse transcription response: {}", e))
        })?;

        // Convert the remote segments to our TranscriptSegment format
        let segments: Vec<TranscriptSegment> = result
            .segments
            .into_iter()
            .map(|seg| TranscriptSegment {
                start_ms: seg.start_ms as u64,
                end_ms: seg.end_ms as u64,
                text: seg.text,
            })
            .collect();

        // If no segments returned, create a single segment from the full text
        if segments.is_empty() {
            Ok(vec![TranscriptSegment {
                start_ms: 0,
                end_ms: (result.duration * 1000.0) as u64,
                text: result.text,
            }])
        } else {
            Ok(segments)
        }
    }
}
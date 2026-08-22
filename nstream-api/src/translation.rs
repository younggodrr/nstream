use crate::models::TranscriptSegmentResponse;

#[derive(Clone)]
pub struct TranslationProvider {
    client: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
}

impl TranslationProvider {
    pub fn from_env() -> Result<Self, String> {
        let endpoint = std::env::var("TRANSLATION_API_URL")
            .map_err(|_| "TRANSLATION_API_URL is required for translation".to_string())?;
        Ok(Self {
            client: reqwest::Client::new(),
            endpoint,
            api_key: std::env::var("TRANSLATION_API_KEY").ok(),
        })
    }

    pub async fn translate(
        &self,
        source: &str,
        target: &str,
        segments: &[TranscriptSegmentResponse],
    ) -> Result<Vec<TranscriptSegmentResponse>, String> {
        let mut translated = Vec::with_capacity(segments.len());
        for segment in segments {
            let response = self.client.post(&self.endpoint).json(&serde_json::json!({
                "q": segment.text, "source": source, "target": target, "format": "text", "api_key": self.api_key,
            })).send().await.map_err(|e| format!("translation request failed: {e}"))?;
            if !response.status().is_success() {
                return Err(format!(
                    "translation service returned {}",
                    response.status()
                ));
            }
            let body: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("invalid translation response: {e}"))?;
            let text = body
                .get("translatedText")
                .and_then(|v| v.as_str())
                .ok_or("translation response missing translatedText")?
                .to_string();
            translated.push(TranscriptSegmentResponse {
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                language: Some(target.to_string()),
                text,
            });
        }
        Ok(translated)
    }
}

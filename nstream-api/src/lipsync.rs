use std::path::Path;

/// HTTP adapter for a deployed lip-sync model (for example a GPU Wav2Lip or
/// MuseTalk service). The service receives a rendered MP4 and returns the
/// lip-synchronised MP4 bytes; it is isolated behind a strict timeout.
pub struct LipSyncProvider {
    endpoint: Option<String>,
    api_key: Option<String>,
}

impl LipSyncProvider {
    pub fn from_env() -> Result<Self, String> {
        let endpoint = std::env::var("LIPSYNC_API_URL").ok();
        if std::env::var("LIPSYNC_REQUIRED").as_deref() == Ok("true") && endpoint.is_none() {
            return Err("LIPSYNC_REQUIRED is true but LIPSYNC_API_URL is not configured".into());
        }
        Ok(Self {
            endpoint,
            api_key: std::env::var("LIPSYNC_API_KEY").ok(),
        })
    }

    pub fn enabled(&self) -> bool {
        self.endpoint.is_some()
    }

    pub async fn process(&self, source: &Path, language: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(endpoint) = &self.endpoint else {
            return Ok(None);
        };
        let bytes = tokio::fs::read(source).await.map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15 * 60))
            .build()
            .map_err(|e| e.to_string())?;
        let mut request = client
            .post(endpoint)
            .header(reqwest::header::CONTENT_TYPE, "video/mp4")
            .header("X-NStream-Target-Language", language)
            .body(bytes);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("lip-sync request failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("lip-sync service returned {}", response.status()));
        }
        let bytes = response.bytes().await.map_err(|e| e.to_string())?.to_vec();
        if bytes.len() < 16 || !bytes.windows(4).any(|window| window == b"ftyp") {
            return Err("lip-sync service did not return an MP4 artifact".into());
        }
        Ok(Some(bytes))
    }
}

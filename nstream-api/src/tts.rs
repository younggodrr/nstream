use std::{path::PathBuf, process::Stdio};
use tokio::io::AsyncWriteExt;

/// Piper is a CPU-friendly, local TTS backend. Voice paths are configured per language:
/// `PIPER_VOICE_EN=/models/en.onnx`, `PIPER_VOICE_SW=/models/sw.onnx`, etc.
#[derive(Clone)]
pub struct PiperTts {
    binary: PathBuf,
}

impl PiperTts {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            binary: std::env::var("PIPER_BIN")
                .map(PathBuf::from)
                .map_err(|_| "PIPER_BIN is required for TTS".to_string())?,
        })
    }
    pub async fn synthesize(&self, text: &str, language: &str) -> Result<Vec<u8>, String> {
        let key = format!(
            "PIPER_VOICE_{}",
            language.to_ascii_uppercase().replace('-', "_")
        );
        let model = std::env::var(&key).map_err(|_| format!("{key} is required for TTS"))?;
        let output_path =
            std::env::temp_dir().join(format!("nstream-tts-{}.wav", uuid::Uuid::new_v4()));
        let mut child = tokio::process::Command::new(&self.binary)
            .args([
                "--model",
                &model,
                "--output_file",
                output_path.to_str().ok_or("invalid temp path")?,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not start Piper: {e}"))?;
        child
            .stdin
            .as_mut()
            .ok_or("Piper stdin unavailable")?
            .write_all(text.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "Piper failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let audio = tokio::fs::read(&output_path)
            .await
            .map_err(|e| e.to_string())?;
        let _ = tokio::fs::remove_file(&output_path).await;
        Ok(audio)
    }
}

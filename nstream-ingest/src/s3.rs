use aws_sdk_s3::Client;
use bytes::Bytes;
use std::path::{Component, Path};
use tokio::{fs, io::AsyncReadExt};
use tracing::info;

use nstream_core::{JobId, MediaPayload, NStreamError};

/// Ingests media from S3 or from a safe local development adapter.
pub struct S3Ingest {
    backend: Backend,
}

enum Backend {
    Local { base_path: String },
    S3 { client: Client },
}

impl S3Ingest {
    pub fn new(_use_local_fallback: bool, local_base_path: impl Into<String>) -> Self {
        let base_path = local_base_path.into();
        Self {
            backend: Backend::Local { base_path },
        }
    }

    pub async fn from_env(local_base_path: impl Into<String>) -> Result<Self, NStreamError> {
        if std::env::var("NSTREAM_STORAGE_BACKEND").unwrap_or_else(|_| "local".into()) != "s3" {
            return Ok(Self {
                backend: Backend::Local {
                    base_path: local_base_path.into(),
                },
            });
        }
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into());
        let shared = aws_config::from_env()
            .region(aws_sdk_s3::config::Region::new(region))
            .load()
            .await;
        let mut config = aws_sdk_s3::config::Builder::from(&shared);
        if let Ok(endpoint) = std::env::var("S3_ENDPOINT_URL") {
            config = config.endpoint_url(endpoint).force_path_style(true);
        }
        Ok(Self {
            backend: Backend::S3 {
                client: Client::from_conf(config.build()),
            },
        })
    }

    /// Ingest a media object from the configured local or S3 backend.
    /// Returns a `MediaPayload` with raw file bytes.
    pub async fn ingest(&self, bucket: &str, key: &str) -> Result<MediaPayload, NStreamError> {
        safe_parts(bucket, key)?;
        match &self.backend {
            Backend::Local { base_path } => self.ingest_local(base_path, bucket, key).await,
            Backend::S3 { client } => {
                let object = client
                    .get_object()
                    .bucket(bucket)
                    .key(key)
                    .send()
                    .await
                    .map_err(|e| NStreamError::Storage(format!("S3 get failed: {e}")))?;
                let mut body = object.body.into_async_read();
                let mut data = Vec::new();
                body.read_to_end(&mut data)
                    .await
                    .map_err(|e| NStreamError::Storage(format!("S3 read failed: {e}")))?;
                Ok(Self::payload(bucket, key, data))
            }
        }
    }

    /// Read from local filesystem: `<local_base_path>/<bucket>/<key>`.
    async fn ingest_local(
        &self,
        base_path: &str,
        bucket: &str,
        key: &str,
    ) -> Result<MediaPayload, NStreamError> {
        safe_parts(bucket, key)?;
        let path = if base_path.is_empty() {
            format!("{}/{}", bucket, key)
        } else {
            format!("{}/{}/{}", base_path, bucket, key)
        };

        info!(path = %path, "S3Ingest reading from local path");

        let data = fs::read(&path)
            .await
            .map_err(|e| NStreamError::Storage(format!("Failed to read '{}': {}", path, e)))?;

        Ok(Self::payload(bucket, key, data))
    }

    fn payload(bucket: &str, key: &str, data: Vec<u8>) -> MediaPayload {
        let job_id = JobId::new();
        let payload = MediaPayload::new(job_id, Bytes::from(data))
            .with_metadata("ingest_source", "s3")
            .with_metadata("s3_bucket", bucket)
            .with_metadata("s3_key", key);

        info!(
            job_id = %job_id,
            bucket = bucket,
            key = key,
            bytes = payload.raw_data.len(),
            "Created MediaPayload from S3 ingest"
        );

        payload
    }
}

fn safe_parts(bucket: &str, key: &str) -> Result<(), NStreamError> {
    if bucket.is_empty()
        || bucket.contains(['/', '\\'])
        || Path::new(key).is_absolute()
        || Path::new(key)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(NStreamError::Storage("unsafe object location".into()));
    }
    Ok(())
}

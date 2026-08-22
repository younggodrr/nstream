use aws_sdk_s3::{presigning::PresigningConfig, Client};
use std::{
    path::{Component, Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[derive(Clone)]
pub enum ObjectStore {
    Local {
        input_root: PathBuf,
        output_root: PathBuf,
    },
    S3 {
        client: Client,
        output_bucket: String,
    },
}

pub struct StagedInput {
    pub path: PathBuf,
    pub temporary: bool,
}

impl ObjectStore {
    pub async fn from_env(input_root: PathBuf, output_root: PathBuf) -> Result<Self, String> {
        if std::env::var("NSTREAM_STORAGE_BACKEND").unwrap_or_else(|_| "local".into()) != "s3" {
            return Ok(Self::Local {
                input_root,
                output_root,
            });
        }
        let output_bucket = std::env::var("S3_BUCKET")
            .map_err(|_| "S3_BUCKET is required for S3 storage".to_string())?;
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into());
        let shared = aws_config::from_env()
            .region(aws_sdk_s3::config::Region::new(region))
            .load()
            .await;
        let mut builder = aws_sdk_s3::config::Builder::from(&shared);
        if let Ok(endpoint) = std::env::var("S3_ENDPOINT_URL") {
            builder = builder.endpoint_url(endpoint).force_path_style(true);
        }
        Ok(Self::S3 {
            client: Client::from_conf(builder.build()),
            output_bucket,
        })
    }

    /// Returns a safe source path only for the development-local backend.
    pub fn local_source_path(&self, bucket: &str, key: &str) -> Result<Option<PathBuf>, String> {
        match self {
            Self::Local { input_root, .. } => Ok(Some(local_path(input_root, bucket, key)?)),
            Self::S3 { .. } => Ok(None),
        }
    }

    /// In S3 mode, user-submitted jobs may only consume objects created through
    /// this tenant's signed-upload prefix. Operators can opt into external
    /// ingestion with NSTREAM_ALLOW_EXTERNAL_INPUT=true.
    pub fn validate_tenant_input(
        &self,
        tenant_id: Uuid,
        bucket: &str,
        key: &str,
    ) -> Result<(), String> {
        match self {
            Self::Local { .. } => Ok(()),
            Self::S3 { .. }
                if std::env::var("NSTREAM_ALLOW_EXTERNAL_INPUT").as_deref() == Ok("true") =>
            {
                Ok(())
            }
            Self::S3 { output_bucket, .. } => {
                let prefix = format!("tenants/{tenant_id}/uploads/");
                if bucket == output_bucket && key.starts_with(&prefix) {
                    Ok(())
                } else {
                    Err("S3 source must be a tenant-scoped object created by /uploads".into())
                }
            }
        }
    }

    pub async fn stage_input(&self, bucket: &str, key: &str) -> Result<StagedInput, String> {
        match self {
            Self::Local { input_root, .. } => Ok(StagedInput {
                path: local_path(input_root, bucket, key)?,
                temporary: false,
            }),
            Self::S3 { client, .. } => {
                let object = client
                    .get_object()
                    .bucket(bucket)
                    .key(key)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                let path = std::env::temp_dir().join(format!("nstream-stage-{}", Uuid::new_v4()));
                let mut file = tokio::fs::File::create(&path)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut body = object.body.into_async_read();
                tokio::io::copy(&mut body, &mut file)
                    .await
                    .map_err(|e| e.to_string())?;
                file.flush().await.map_err(|e| e.to_string())?;
                Ok(StagedInput {
                    path,
                    temporary: true,
                })
            }
        }
    }

    pub async fn stage_artifact(&self, uri: &str) -> Result<StagedInput, String> {
        if let Some(path) = uri.strip_prefix("file://") {
            return Ok(StagedInput {
                path: PathBuf::from(path),
                temporary: false,
            });
        }
        let (bucket, key) = parse_s3_uri(uri)?;
        self.stage_input(bucket, key).await
    }

    pub async fn deliver(
        &self,
        input_bucket: &str,
        input_key: &str,
        output_key: &str,
    ) -> Result<String, String> {
        match self {
            Self::Local {
                input_root,
                output_root,
            } => {
                let input = local_path(input_root, input_bucket, input_key)?;
                let output = local_path(output_root, "delivery", output_key)?;
                let parent = output.parent().ok_or("invalid output key")?;
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| e.to_string())?;
                let temporary = parent.join(format!(
                    ".{}.part",
                    output
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("artifact")
                ));
                tokio::fs::copy(input, &temporary)
                    .await
                    .map_err(|e| e.to_string())?;
                tokio::fs::rename(temporary, &output)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("file://{}", output.display()))
            }
            Self::S3 {
                client,
                output_bucket,
            } => {
                client
                    .copy_object()
                    .copy_source(format!("{input_bucket}/{input_key}"))
                    .bucket(output_bucket)
                    .key(output_key)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("s3://{output_bucket}/{output_key}"))
            }
        }
    }

    pub async fn store_audio(&self, key: &str, audio: Vec<u8>) -> Result<String, String> {
        match self {
            Self::Local { output_root, .. } => {
                let output = local_path(output_root, "delivery", key)?;
                let parent = output.parent().ok_or("invalid audio key")?;
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| e.to_string())?;
                tokio::fs::write(&output, audio)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("file://{}", output.display()))
            }
            Self::S3 {
                client,
                output_bucket,
            } => {
                client
                    .put_object()
                    .bucket(output_bucket)
                    .key(key)
                    .content_type("audio/wav")
                    .body(aws_sdk_s3::primitives::ByteStream::from(audio))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("s3://{output_bucket}/{key}"))
            }
        }
    }

    pub async fn store_file(
        &self,
        key: &str,
        source: &Path,
        content_type: &str,
    ) -> Result<String, String> {
        match self {
            Self::Local { output_root, .. } => {
                let output = local_path(output_root, "delivery", key)?;
                let parent = output.parent().ok_or("invalid output key")?;
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| e.to_string())?;
                let temporary = parent.join(format!(
                    ".{}.part",
                    output
                        .file_name()
                        .and_then(|v| v.to_str())
                        .unwrap_or("artifact")
                ));
                tokio::fs::copy(source, &temporary)
                    .await
                    .map_err(|e| e.to_string())?;
                tokio::fs::rename(temporary, &output)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("file://{}", output.display()))
            }
            Self::S3 {
                client,
                output_bucket,
            } => {
                let bytes = tokio::fs::read(source).await.map_err(|e| e.to_string())?;
                client
                    .put_object()
                    .bucket(output_bucket)
                    .key(key)
                    .content_type(content_type)
                    .body(aws_sdk_s3::primitives::ByteStream::from(bytes))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format!("s3://{output_bucket}/{key}"))
            }
        }
    }

    pub async fn presign_get(
        &self,
        artifact: &str,
        expires_in: Duration,
    ) -> Result<Option<String>, String> {
        let Self::S3 { client, .. } = self else {
            return Ok(None);
        };
        let (bucket, key) = parse_s3_uri(artifact)?;
        let config = PresigningConfig::expires_in(expires_in).map_err(|e| e.to_string())?;
        let request = client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(config)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Some(request.uri().to_string()))
    }

    pub async fn presign_put(
        &self,
        key: &str,
        content_type: &str,
        expires_in: Duration,
    ) -> Result<Option<(String, String)>, String> {
        let Self::S3 {
            client,
            output_bucket,
        } = self
        else {
            return Ok(None);
        };
        let config = PresigningConfig::expires_in(expires_in).map_err(|e| e.to_string())?;
        let request = client
            .put_object()
            .bucket(output_bucket)
            .key(key)
            .content_type(content_type)
            .presigned(config)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Some((output_bucket.clone(), request.uri().to_string())))
    }
}

fn local_path(root: &Path, bucket: &str, key: &str) -> Result<PathBuf, String> {
    if bucket.is_empty() || bucket.contains('/') || bucket.contains('\\') {
        return Err("bucket must be one path component".into());
    }
    let path = Path::new(key);
    if path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err("object key must be a safe relative path".into());
    }
    Ok(root.join(bucket).join(path))
}

fn parse_s3_uri(uri: &str) -> Result<(&str, &str), String> {
    let value = uri
        .strip_prefix("s3://")
        .ok_or("artifact is not an S3 URI")?;
    value.split_once('/').ok_or("invalid S3 URI".to_string())
}

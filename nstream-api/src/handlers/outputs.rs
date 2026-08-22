use std::path::Path;

use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

use nstream_core::JobId;

use crate::{
    models::{ArtifactAccessResponse, ErrorResponse},
    state::AppState,
};

/// GET /jobs/:id/outputs/:language
/// Streams a completed local-delivery artifact for the requested language.
pub async fn download_output(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, language)): AxumPath<(uuid::Uuid, String)>,
) -> Result<Response, (StatusCode, axum::Json<ErrorResponse>)> {
    let job_id = JobId(id);
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let artifact = {
        let job = state
            .store
            .get_job(tenant_id, job_id)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(ErrorResponse {
                        error: "storage_error".to_string(),
                        message: error.to_string(),
                    }),
                )
            })?
            .ok_or_else(|| not_found("job", &job_id.to_string()))?;
        job.output_files
            .iter()
            .find(|path| {
                Path::new(path)
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    == Some(language.as_str())
            })
            .cloned()
            .ok_or_else(|| not_found("output", &language))?
    };

    let local_path = artifact.strip_prefix("file://").unwrap_or(&artifact);
    let bytes = tokio::fs::read(local_path)
        .await
        .map_err(|_| not_found("output", &language))?;
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream")],
        Body::from(bytes),
    )
        .into_response())
}

fn not_found(resource: &str, value: &str) -> (StatusCode, axum::Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        axum::Json(ErrorResponse {
            error: "not_found".to_string(),
            message: format!("{resource} '{value}' not found"),
        }),
    )
}

/// GET /jobs/:id/outputs/:language/url -- a 10-minute S3 presigned download URL.
pub async fn signed_output_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, language)): AxumPath<(uuid::Uuid, String)>,
) -> Result<axum::Json<ArtifactAccessResponse>, (StatusCode, axum::Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let job = state
        .store
        .get_job(tenant_id, JobId(id))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    error: "storage_error".into(),
                    message: e.to_string(),
                }),
            )
        })?
        .ok_or_else(|| not_found("job", &id.to_string()))?;
    let artifact = job
        .output_files
        .iter()
        .find(|path| path.contains(&format!("/{language}/")))
        .ok_or_else(|| not_found("output", &language))?;
    let url = state
        .storage
        .presign_get(artifact, std::time::Duration::from_secs(600))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    error: "storage_error".into(),
                    message: e,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::CONFLICT,
                axum::Json(ErrorResponse {
                    error: "unsupported".into(),
                    message: "signed URLs require NSTREAM_STORAGE_BACKEND=s3".into(),
                }),
            )
        })?;
    Ok(axum::Json(ArtifactAccessResponse {
        url,
        expires_in_seconds: 600,
    }))
}

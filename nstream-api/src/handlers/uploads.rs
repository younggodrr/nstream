use crate::{
    models::{CreateUploadRequest, CreateUploadResponse, ErrorResponse},
    state::AppState,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use uuid::Uuid;

/// POST /uploads -- create a tenant-scoped, 15-minute upload URL.
pub async fn create_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateUploadRequest>,
) -> Result<Json<CreateUploadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let file_name = std::path::Path::new(&request.file_name)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "validation_error".into(),
                    message: "file_name must be a file name".into(),
                }),
            )
        })?;
    if !request.content_type.starts_with("video/") && !request.content_type.starts_with("audio/") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_error".into(),
                message: "content_type must be audio or video".into(),
            }),
        ));
    }
    let key = format!(
        "tenants/{tenant_id}/uploads/{}/{}",
        Uuid::new_v4(),
        file_name
    );
    let (bucket, upload_url) = state
        .storage
        .presign_put(
            &key,
            &request.content_type,
            std::time::Duration::from_secs(900),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "storage_error".into(),
                    message: e,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "unsupported".into(),
                    message: "uploads require NSTREAM_STORAGE_BACKEND=s3".into(),
                }),
            )
        })?;
    Ok(Json(CreateUploadResponse {
        bucket,
        key,
        upload_url,
        expires_in_seconds: 900,
    }))
}

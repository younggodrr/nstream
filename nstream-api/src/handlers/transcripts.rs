use crate::{
    models::{ErrorResponse, TranscriptSegmentResponse},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use nstream_core::JobId;

/// GET /jobs/:id/transcript -- timestamped source-language ASR segments.
pub async fn get_transcript(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Vec<TranscriptSegmentResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let segments = state
        .store
        .transcript(tenant_id, JobId(id))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "storage_error".into(),
                    message: e.to_string(),
                }),
            )
        })?;
    Ok(Json(
        segments
            .into_iter()
            .map(|segment| TranscriptSegmentResponse {
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                language: segment.language,
                text: segment.text,
            })
            .collect(),
    ))
}

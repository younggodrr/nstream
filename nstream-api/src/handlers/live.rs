use crate::{
    models::{ErrorResponse, LiveSessionResponse, StartLiveSessionRequest},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};

pub async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<StartLiveSessionRequest>,
) -> Result<(StatusCode, Json<LiveSessionResponse>), (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let (id, playlist, publish_status) = state
        .live_sessions
        .start(tenant_id, request.source_url, request.publish_url)
        .await
        .map_err(internal)?;
    Ok((
        StatusCode::CREATED,
        Json(response(id, "running", playlist, publish_status)),
    ))
}

pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<LiveSessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    match state
        .live_sessions
        .status(tenant_id, id)
        .await
        .map_err(internal)?
    {
        Some((running, playlist, publishing)) => Ok(Json(response(
            id,
            if running { "running" } else { "stopped" },
            playlist,
            publishing.map(|active| {
                if active {
                    "running".into()
                } else {
                    "stopped".into()
                }
            }),
        ))),
        None => Err(not_found()),
    }
}

pub async fn stop(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    if state
        .live_sessions
        .stop(tenant_id, id)
        .await
        .map_err(internal)?
    {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found())
    }
}

fn response(
    id: uuid::Uuid,
    status: &str,
    playlist: std::path::PathBuf,
    publish_status: Option<String>,
) -> LiveSessionResponse {
    LiveSessionResponse {
        session_id: id.to_string(),
        status: status.into(),
        playlist_path: playlist.display().to_string(),
        publish_status,
    }
}
fn not_found() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not_found".into(),
            message: "live session not found".into(),
        }),
    )
}
fn internal(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "live_ingest_error".into(),
            message,
        }),
    )
}

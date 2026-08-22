use crate::{models::ErrorResponse, state::AppState};
use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn key_hash(key: &str) -> Vec<u8> {
    Sha256::digest(key.as_bytes()).to_vec()
}

pub async fn tenant_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Uuid, (axum::http::StatusCode, axum::Json<ErrorResponse>)> {
    if !state.auth_required {
        return Ok(state.tenant_id);
    }
    let key = headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| unauthorized("missing x-api-key"))?;
    state
        .store
        .authenticate_api_key(key)
        .await
        .map_err(|_| unauthorized("invalid API key"))?
        .ok_or_else(|| unauthorized("invalid API key"))
}

fn unauthorized(message: &str) -> (axum::http::StatusCode, axum::Json<ErrorResponse>) {
    (
        axum::http::StatusCode::UNAUTHORIZED,
        axum::Json(ErrorResponse {
            error: "unauthorized".into(),
            message: message.into(),
        }),
    )
}

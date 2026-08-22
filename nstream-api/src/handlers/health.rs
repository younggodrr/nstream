use axum::response::Json;
use axum::{extract::State, http::StatusCode};
use serde_json::{json, Value};

/// GET /health
/// Returns a simple health-check response.
pub async fn health_check() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "nstream-api" }))
}

/// GET /readyz -- readiness includes the durable control plane.
pub async fn readiness(
    State(state): State<crate::state::AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state.store.is_healthy().await.map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "not_ready", "reason": "database_unavailable" })),
        )
    })?;
    Ok(Json(json!({ "status": "ready" })))
}

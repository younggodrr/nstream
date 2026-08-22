use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use prometheus::{Encoder, TextEncoder};

use crate::state::AppState;

/// GET /metrics -- Prometheus text exposition format.
pub async fn metrics(state: axum::extract::State<AppState>) -> Response {
    let encoder = TextEncoder::new();
    let families = state.registry.gather();
    match encoder.encode_to_string(&families) {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, encoder.format_type())],
            body,
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode metrics: {error}"),
        )
            .into_response(),
    }
}

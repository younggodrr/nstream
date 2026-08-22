use axum::{
    http::HeaderValue,
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

use crate::handlers::{health, jobs, live, metrics, outputs, transcripts, uploads, webhooks};
use crate::state::AppState;

/// Build the full axum router with all routes and middleware.
pub fn build_router(state: AppState) -> Router {
    let cors = std::env::var("CORS_ALLOW_ORIGIN")
        .ok()
        .and_then(|origin| origin.parse::<HeaderValue>().ok())
        .map(|origin| {
            CorsLayer::new()
                .allow_origin(origin)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::DELETE,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::HeaderName::from_static("x-api-key"),
                ])
        });

    let router = Router::new()
        // Health
        .route("/health", get(health::health_check))
        .route("/readyz", get(health::readiness))
        .route("/metrics", get(metrics::metrics))
        // Jobs
        .route("/jobs", post(jobs::submit_job))
        .route("/jobs", get(jobs::list_jobs))
        .route("/jobs/:id", get(jobs::get_job))
        .route("/jobs/:id", axum::routing::delete(jobs::cancel_job))
        .route("/jobs/:id/stages", get(jobs::get_job_stages))
        .route("/jobs/:id/events", get(jobs::get_job_events))
        .route("/jobs/:id/transcript", get(transcripts::get_transcript))
        .route("/uploads", post(uploads::create_upload))
        .route("/live", post(live::start))
        .route("/live/:id", get(live::status).delete(live::stop))
        .route("/jobs/:id/outputs/:language", get(outputs::download_output))
        .route(
            "/jobs/:id/outputs/:language/url",
            get(outputs::signed_output_url),
        )
        // Webhooks
        .route("/webhooks/register", post(webhooks::register_webhook))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::rate_limit::enforce,
        ))
        .with_state(state);
    match cors {
        Some(cors) => router.layer(cors),
        None => router,
    }
}

/// Start the axum HTTP server on the given address string (e.g. "0.0.0.0:8080").
pub async fn run(addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = AppState::new().await?;
    let router = build_router(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(addr = addr, "NStream API server listening");

    axum::serve(listener, router).await?;
    Ok(())
}

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use tracing::{info, warn};

use crate::models::{
    ErrorResponse, JobEventResponse, JobResponse, JobStageResponse, SubmitJobRequest,
};
use crate::state::AppState;
use nstream_core::{JobId, JobMetadata};

/// Convert a `JobMetadata` record into the API response shape.
fn job_to_response(meta: &JobMetadata) -> JobResponse {
    JobResponse {
        job_id: meta.job_id,
        status: meta.status.clone(),
        source_lang: meta.source_lang.clone(),
        target_langs: meta.target_langs.clone(),
        pipeline_mode: meta.pipeline_mode.clone(),
        created_at: meta.created_at,
        updated_at: meta.updated_at,
        s3_bucket: meta.s3_bucket.clone(),
        s3_key: meta.s3_key.clone(),
        output_files: meta.output_files.clone(),
        error_message: meta.error_message.clone(),
        scene_ranges: meta.scene_ranges.clone(),
    }
}

/// POST /jobs
/// Submit a new processing job. Assigns a job ID and stores metadata with Pending status.
pub async fn submit_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SubmitJobRequest>,
) -> Result<(StatusCode, Json<JobResponse>), (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    if req.source_lang.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_error".to_string(),
                message: "source_lang must not be empty".to_string(),
            }),
        ));
    }

    if req.target_langs.is_empty() || req.target_langs.iter().any(|lang| lang.trim().is_empty()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_error".to_string(),
                message: "target_langs must contain at least one language".to_string(),
            }),
        ));
    }
    let scene_ranges = req.scene_ranges.clone().unwrap_or_default();
    if scene_ranges
        .iter()
        .any(|range| range.start_ms >= range.end_ms)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_error".into(),
                message: "each scene range must have start_ms before end_ms".into(),
            }),
        ));
    }

    if req
        .s3_bucket
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        || req.s3_key.as_deref().unwrap_or_default().trim().is_empty()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_error".to_string(),
                message: "s3_bucket and s3_key are required; create a signed upload first when using S3 storage"
                    .to_string(),
            }),
        ));
    }
    state
        .storage
        .validate_tenant_input(
            tenant_id,
            req.s3_bucket.as_deref().expect("validated source bucket"),
            req.s3_key.as_deref().expect("validated source key"),
        )
        .map_err(|message| {
            (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "tenant_isolation".into(),
                    message,
                }),
            )
        })?;

    let job_id = JobId::new();
    let mut meta = JobMetadata::new(
        job_id,
        req.source_lang.clone(),
        req.target_langs.clone(),
        req.pipeline_mode.clone(),
    );

    meta = meta.with_s3(
        req.s3_bucket.clone().expect("validated above"),
        req.s3_key.clone().expect("validated above"),
    );
    meta = meta.with_scene_ranges(scene_ranges);

    if let Err(error) = state.store.create_job(tenant_id, &meta).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "storage_error".to_string(),
                message: format!("job could not be stored: {error}"),
            }),
        ));
    }
    let _ = state
        .store
        .record_event(
            job_id,
            "submitted",
            serde_json::json!({
                "pipeline_mode": meta.pipeline_mode,
                "target_langs": meta.target_langs,
            }),
        )
        .await;

    state.metrics.record_job_submitted();

    info!(
        job_id = %job_id,
        source_lang = %req.source_lang,
        pipeline_mode = %req.pipeline_mode,
        "Job submitted"
    );

    Ok((StatusCode::CREATED, Json(job_to_response(&meta))))
}

/// GET /jobs/:id
/// Poll the status of an existing job.
pub async fn get_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<JobResponse>, (StatusCode, Json<ErrorResponse>)> {
    let job_id = JobId(id);
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    match state.store.get_job(tenant_id, job_id).await {
        Ok(Some(meta)) => {
            info!(job_id = %job_id, status = %meta.status, "Job status polled");
            Ok(Json(job_to_response(&meta)))
        }
        Ok(None) => {
            warn!(job_id = %job_id, "Job not found");
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "not_found".to_string(),
                    message: format!("Job {} not found", job_id),
                }),
            ))
        }
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "storage_error".to_string(),
                message: error.to_string(),
            }),
        )),
    }
}

/// DELETE /jobs/:id
/// Stop a pending/running job owned by the authenticated tenant.
pub async fn cancel_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    match state.store.cancel(tenant_id, JobId(id)).await {
        Ok(true) => {
            let _ = state
                .store
                .fail_active_stages(JobId(id), "cancelled by user")
                .await;
            let _ = state
                .store
                .record_event(
                    JobId(id),
                    "cancelled",
                    serde_json::json!({ "reason": "cancelled by user" }),
                )
                .await;
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(false) => Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "not_cancellable".into(),
                message: "job was not found or is already terminal".into(),
            }),
        )),
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "storage_error".into(),
                message: error.to_string(),
            }),
        )),
    }
}

/// GET /jobs
/// List all known jobs with their current statuses.
pub async fn list_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<JobResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let jobs = state.store.list_jobs(tenant_id).await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "storage_error".to_string(),
                message: error.to_string(),
            }),
        )
    })?;
    let list = jobs.iter().map(job_to_response).collect::<Vec<_>>();
    info!(count = list.len(), "Listing all jobs");
    Ok(Json(list))
}

/// GET /jobs/:id/stages
pub async fn get_job_stages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Vec<JobStageResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let stages = state
        .store
        .stages(tenant_id, JobId(id))
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "storage_error".into(),
                    message: error.to_string(),
                }),
            )
        })?;
    Ok(Json(
        stages
            .into_iter()
            .map(|stage| JobStageResponse {
                stage: stage.stage,
                status: stage.status,
                detail: stage.detail,
                started_at: stage.started_at,
                completed_at: stage.completed_at,
            })
            .collect(),
    ))
}

/// GET /jobs/:id/events
pub async fn get_job_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Vec<JobEventResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let events = state
        .store
        .events(tenant_id, JobId(id))
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "storage_error".into(),
                    message: error.to_string(),
                }),
            )
        })?;
    Ok(Json(
        events
            .into_iter()
            .map(|event| JobEventResponse {
                event_type: event.event_type,
                payload: event.payload,
                created_at: event.created_at,
            })
            .collect(),
    ))
}

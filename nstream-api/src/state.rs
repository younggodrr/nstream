use std::{path::PathBuf, sync::Arc, time::Instant};

use prometheus::Registry;
use tokio::time::{sleep, Duration};
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    handlers::webhooks::dispatch_webhooks, persistence::PostgresStore, storage::ObjectStore,
};
use nstream_core::JobMetadata;
use nstream_inference::WhisperAsr;
use nstream_media::{
    extract_asr_audio_chunk, probe_duration_ms, render_dubbed_video, validate_rendered_mp4,
};
use nstream_observe::{register, NStreamMetrics};

#[derive(Clone)]
pub struct AppState {
    pub store: PostgresStore,
    pub tenant_id: Uuid,
    pub metrics: Arc<NStreamMetrics>,
    pub registry: Arc<Registry>,
    pub storage: Arc<ObjectStore>,
    pub auth_required: bool,
    pub rate_limiter: Arc<crate::rate_limit::RateLimiter>,
    pub live_sessions: Arc<crate::live::LiveSessionRegistry>,
    pub webhook_encryption_key: Option<Arc<String>>,
}

impl AppState {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let database_url = std::env::var("DATABASE_URL")?;
        let store = PostgresStore::connect(&database_url).await?;
        let tenant_id = std::env::var("NSTREAM_DEFAULT_TENANT_ID")
            .ok()
            .and_then(|id| Uuid::parse_str(&id).ok())
            .unwrap_or(Uuid::nil());
        store.ensure_tenant(tenant_id, "local-default").await?;
        let auth_required = std::env::var("NSTREAM_AUTH_REQUIRED")
            .map(|v| v == "true")
            .unwrap_or(false);
        let webhook_encryption_key = std::env::var("WEBHOOK_ENCRYPTION_KEY").ok().map(Arc::new);
        if auth_required && webhook_encryption_key.is_none() {
            return Err("WEBHOOK_ENCRYPTION_KEY is required when authentication is enabled".into());
        }
        if let Ok(key) = std::env::var("NSTREAM_BOOTSTRAP_API_KEY") {
            store.bootstrap_api_key(tenant_id, &key).await?;
        }
        let (registry, metrics) = register();
        let input_root = std::env::var("NSTREAM_LOCAL_MEDIA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./media-input"));
        let output_root = std::env::var("NSTREAM_OUTPUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./media-output"));
        let storage = ObjectStore::from_env(input_root, output_root)
            .await
            .map_err(|e| format!("storage configuration error: {e}"))?;
        let state = Self {
            store,
            tenant_id,
            metrics: Arc::new(metrics),
            registry: Arc::new(registry),
            storage: Arc::new(storage),
            auth_required,
            rate_limiter: Arc::new(crate::rate_limit::RateLimiter::from_env()),
            live_sessions: Arc::new(crate::live::LiveSessionRegistry::from_env()),
            webhook_encryption_key,
        };
        Ok(state)
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v: &usize| *v > 0)
        .unwrap_or(default)
}

pub fn start_workers(state: AppState) {
    spawn_workers(state, env_usize("JOB_WORKER_COUNT", 2));
}

fn spawn_workers(state: AppState, worker_count: usize) {
    for number in 0..worker_count {
        let state = state.clone();
        tokio::spawn(async move {
            let worker_id = format!("{}-{}", Uuid::new_v4(), number);
            loop {
                if let Err(error) = state.store.recover_expired_leases().await {
                    error!(%error, "lease recovery failed");
                }
                match state.store.claim_next(&worker_id, 60).await {
                    Ok(Some(claimed)) => {
                        state.metrics.active_gpu_workers.inc();
                        run_job(&state, &worker_id, claimed.tenant_id, claimed.job).await;
                        state.metrics.active_gpu_workers.dec();
                    }
                    Ok(None) => sleep(Duration::from_millis(500)).await,
                    Err(error) => {
                        error!(%error, "job claim failed");
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });
    }
}

async fn run_job(state: &AppState, worker_id: &str, tenant_id: Uuid, job: JobMetadata) {
    let started = Instant::now();
    let (stop_heartbeat, mut heartbeat_stopped) = tokio::sync::oneshot::channel::<()>();
    let heartbeat_store = state.store.clone();
    let heartbeat_job_id = job.job_id;
    let heartbeat_worker_id = worker_id.to_string();
    let heartbeat = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(20)) => {
                    match heartbeat_store.heartbeat(heartbeat_job_id, &heartbeat_worker_id, 60).await {
                        Ok(true) => {},
                        _ => return,
                    }
                }
                _ = &mut heartbeat_stopped => return,
            }
        }
    });
    let result = async {
        ensure_job_lease(state, job.job_id, worker_id).await?;
        state
            .store
            .start_stage(job.job_id, "asr")
            .await
            .map_err(|e| e.to_string())?;
        let transcript = transcribe_source(state, tenant_id, &job).await?;
        state
            .store
            .replace_transcript(job.job_id, Some(&job.source_lang), &transcript)
            .await
            .map_err(|e| format!("could not persist transcript: {e}"))?;
        state
            .store
            .finish_stage(job.job_id, "asr", Some("transcript persisted"))
            .await
            .map_err(|e| e.to_string())?;
        let source_segments = transcript
            .iter()
            .filter(|segment| {
                job.scene_ranges.is_empty()
                    || job.scene_ranges.iter().any(|range| {
                        segment.start_ms >= range.start_ms && segment.end_ms <= range.end_ms
                    })
            })
            .map(|segment| crate::models::TranscriptSegmentResponse {
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                language: Some(job.source_lang.clone()),
                text: segment.text.clone(),
            })
            .collect::<Vec<_>>();
        let translator = crate::translation::TranslationProvider::from_env()?;
        let tts = crate::tts::PiperTts::from_env()?;
        for target in &job.target_langs {
            ensure_job_lease(state, job.job_id, worker_id).await?;
            state
                .store
                .start_stage(job.job_id, &format!("translation:{target}"))
                .await
                .map_err(|e| e.to_string())?;
            let translated = if target == &job.source_lang {
                source_segments.clone()
            } else {
                translator
                    .translate(&job.source_lang, target, &source_segments)
                    .await?
            };
            state
                .store
                .replace_translation(job.job_id, target, &translated)
                .await
                .map_err(|e| format!("could not persist translation: {e}"))?;
            state
                .store
                .finish_stage(job.job_id, &format!("translation:{target}"), None)
                .await
                .map_err(|e| e.to_string())?;
            state
                .store
                .start_stage(job.job_id, &format!("tts:{target}"))
                .await
                .map_err(|e| e.to_string())?;
            for segment in translated {
                ensure_job_lease(state, job.job_id, worker_id).await?;
                let audio = tts.synthesize(&segment.text, target).await?;
                let audio_key = format!(
                    "tenants/{tenant_id}/jobs/{}/{target}/audio/{}-{}.wav",
                    job.job_id, segment.start_ms, segment.end_ms
                );
                let artifact_uri = state.storage.store_audio(&audio_key, audio).await?;
                state
                    .store
                    .replace_audio_segment(
                        job.job_id,
                        target,
                        segment.start_ms,
                        segment.end_ms,
                        &artifact_uri,
                    )
                    .await
                    .map_err(|e| format!("could not persist synthesized audio: {e}"))?;
            }
            state
                .store
                .finish_stage(job.job_id, &format!("tts:{target}"), None)
                .await
                .map_err(|e| e.to_string())?;
        }
        ensure_job_lease(state, job.job_id, worker_id).await?;
        state
            .store
            .start_stage(job.job_id, "render")
            .await
            .map_err(|e| e.to_string())?;
        let outputs = render_target_artifacts(state, tenant_id, &job).await?;
        state
            .store
            .finish_stage(job.job_id, "render", Some("MP4 artifacts stored"))
            .await
            .map_err(|e| e.to_string())?;
        Ok(outputs)
    }
    .await;
    let _ = stop_heartbeat.send(());
    let _ = heartbeat.await;
    match result {
        Ok(outputs) => match state.store.complete(job.job_id, worker_id, &outputs).await {
            Ok(true) => {
                state
                    .metrics
                    .record_job_completed(started.elapsed().as_secs_f64());
                let mut notification = job.clone();
                notification.output_files = outputs;
                notification.set_status(nstream_core::JobStatus::Completed);
                let _ = state
                    .store
                    .record_event(
                        job.job_id,
                        "completed",
                        serde_json::json!({ "outputs": notification.output_files }),
                    )
                    .await;
                let _ = dispatch_webhooks(state, tenant_id, &notification, "completed").await;
            }
            Ok(false) => warn!(job_id = %job.job_id, "worker lost job lease before completion"),
            Err(error) => error!(%error, "could not complete job"),
        },
        Err(message) => match state.store.fail(job.job_id, worker_id, &message).await {
            Ok(true) => {
                let _ = state.store.fail_active_stages(job.job_id, &message).await;
                state.metrics.record_job_failed();
                let mut notification = job;
                notification.fail(message);
                let _ = state
                    .store
                    .record_event(
                        notification.job_id,
                        "failed",
                        serde_json::json!({ "error": notification.error_message }),
                    )
                    .await;
                let _ = dispatch_webhooks(state, tenant_id, &notification, "failed").await;
            }
            Ok(false) => warn!(job_id = %job.job_id, "worker lost job lease before failure update"),
            Err(error) => error!(%error, "could not persist job failure"),
        },
    }
}

async fn ensure_job_lease(
    state: &AppState,
    job_id: nstream_core::JobId,
    worker_id: &str,
) -> Result<(), String> {
    if state
        .store
        .heartbeat(job_id, worker_id, 60)
        .await
        .map_err(|e| e.to_string())?
    {
        Ok(())
    } else {
        Err("job lease was released (cancelled or claimed by another worker)".into())
    }
}

async fn transcribe_source(
    state: &AppState,
    _tenant_id: Uuid,
    job: &JobMetadata,
) -> Result<Vec<nstream_inference::TranscriptSegment>, String> {
    let bucket = job.s3_bucket.as_deref().ok_or("job has no source bucket")?;
    let key = job.s3_key.as_deref().ok_or("job has no source key")?;
    let staged = state.storage.stage_input(bucket, key).await?;
    let source = staged.path;
    let duration = probe_duration_ms(&source)
        .await
        .map_err(|e| e.to_string())?;
    let model_path =
        std::env::var("WHISPER_MODEL_PATH").unwrap_or_else(|_| "models/ggml-base.bin".to_string());
    let asr = WhisperAsr::new(model_path);
    let mut transcript = Vec::new();
    for start_ms in (0..duration).step_by(30_000) {
        let chunk = extract_asr_audio_chunk(&source, start_ms, (duration - start_ms).min(30_000))
            .await
            .map_err(|e| e.to_string())?;
        let mut segments = asr
            .transcribe_pcm(chunk.samples, Some(job.source_lang.clone()))
            .await
            .map_err(|e| e.to_string())?;
        for segment in &mut segments {
            segment.start_ms += start_ms;
            segment.end_ms += start_ms;
        }
        transcript.extend(segments);
    }
    if staged.temporary {
        let _ = tokio::fs::remove_file(&source).await;
    }
    Ok(transcript)
}

async fn render_target_artifacts(
    state: &AppState,
    tenant_id: Uuid,
    job: &JobMetadata,
) -> Result<Vec<String>, String> {
    let bucket = job.s3_bucket.as_deref().ok_or("job has no source bucket")?;
    let key = job.s3_key.as_deref().ok_or("job has no source key")?;
    let source = state.storage.stage_input(bucket, key).await?;
    let lipsync = crate::lipsync::LipSyncProvider::from_env()?;
    let mut outputs = Vec::with_capacity(job.target_langs.len());
    for language in &job.target_langs {
        if !language
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(format!("invalid target language '{language}'"));
        }
        let rows = state
            .store
            .audio_segments_for_target(job.job_id, language)
            .await
            .map_err(|e| format!("could not load synthesized audio: {e}"))?;
        let mut staged_audio = Vec::with_capacity(rows.len());
        for row in rows {
            let artifact = state.storage.stage_artifact(&row.artifact_uri).await?;
            staged_audio.push((artifact, row.start_ms));
        }
        let render_path =
            std::env::temp_dir().join(format!("nstream-render-{}-{language}.mp4", Uuid::new_v4()));
        let segments = staged_audio
            .iter()
            .map(|(artifact, start_ms)| (artifact.path.clone(), *start_ms))
            .collect::<Vec<_>>();
        let mute_ranges = if matches!(
            job.pipeline_mode,
            nstream_core::PipelineMode::CreatorSelective
        ) {
            job.scene_ranges
                .iter()
                .map(|range| (range.start_ms, range.end_ms))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let rendered =
            render_dubbed_video(&source.path, &segments, &mute_ranges, &render_path).await;
        for (artifact, _) in &staged_audio {
            if artifact.temporary {
                let _ = tokio::fs::remove_file(&artifact.path).await;
            }
        }
        rendered?;
        if let Some(lip_synced) = lipsync.process(&render_path, language).await? {
            tokio::fs::write(&render_path, lip_synced)
                .await
                .map_err(|e| e.to_string())?;
        }
        validate_rendered_mp4(&render_path)
            .await
            .map_err(|e| e.to_string())?;
        let output_key = format!(
            "tenants/{tenant_id}/jobs/{}/{language}/dubbed.mp4",
            job.job_id
        );
        let artifact = state
            .storage
            .store_file(&output_key, &render_path, "video/mp4")
            .await;
        let _ = tokio::fs::remove_file(&render_path).await;
        outputs.push(artifact?);
    }
    if source.temporary {
        let _ = tokio::fs::remove_file(&source.path).await;
    }
    Ok(outputs)
}

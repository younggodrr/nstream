//! PostgreSQL control-plane primitives.  These queries use `FOR UPDATE SKIP LOCKED`
//! so several API/worker replicas can claim work without duplicate processing.
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

use nstream_core::{JobId, JobMetadata, JobStatus, PipelineMode};
use nstream_inference::TranscriptSegment;

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

pub struct ClaimedJob {
    pub tenant_id: Uuid,
    pub job: JobMetadata,
}
pub struct WebhookDelivery {
    pub url: String,
    pub signing_secret: String,
}
pub struct StoredTranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub language: Option<String>,
    pub text: String,
}
pub struct StoredAudioSegment {
    pub start_ms: u64,
    pub artifact_uri: String,
}
pub struct StoredJobStage {
    pub stage: String,
    pub status: String,
    pub detail: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
pub struct StoredJobEvent {
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl PostgresStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn is_healthy(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
    }

    pub async fn ensure_tenant(&self, tenant_id: Uuid, slug: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO tenants (id, slug) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
            .bind(tenant_id)
            .bind(slug)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn bootstrap_api_key(
        &self,
        tenant_id: Uuid,
        raw_key: &str,
    ) -> Result<(), sqlx::Error> {
        let hash = crate::auth::key_hash(raw_key);
        let prefix: String = raw_key.chars().take(12).collect();
        sqlx::query("INSERT INTO api_keys (id, tenant_id, key_prefix, key_hash, name) VALUES ($1, $2, $3, $4, 'bootstrap') ON CONFLICT (key_hash) DO NOTHING")
            .bind(Uuid::new_v4()).bind(tenant_id).bind(prefix).bind(hash).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn authenticate_api_key(&self, raw_key: &str) -> Result<Option<Uuid>, sqlx::Error> {
        let hash = crate::auth::key_hash(raw_key);
        let row = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE key_hash = $1 AND revoked_at IS NULL RETURNING tenant_id")
            .bind(hash).fetch_optional(&self.pool).await?;
        row.map(|row| row.try_get("tenant_id")).transpose()
    }

    pub async fn register_webhook(
        &self,
        tenant_id: Uuid,
        url: &str,
        signing_secret: &str,
        encryption_key: &str,
        events: &[String],
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO webhook_subscriptions (id, tenant_id, url, signing_secret, signing_secret_ciphertext, events) VALUES ($1, $2, $3, NULL, pgp_sym_encrypt($4::text, $5::text), $6)")
            .bind(id).bind(tenant_id).bind(url).bind(signing_secret).bind(encryption_key)
            .bind(serde_json::to_value(events).expect("events serialize")).execute(&self.pool).await?;
        Ok(id)
    }

    pub async fn webhooks_for_event(
        &self,
        tenant_id: Uuid,
        event: &str,
        encryption_key: &str,
    ) -> Result<Vec<WebhookDelivery>, sqlx::Error> {
        let rows = sqlx::query("SELECT url, pgp_sym_decrypt(signing_secret_ciphertext, $3::text) AS signing_secret FROM webhook_subscriptions WHERE tenant_id = $1 AND disabled_at IS NULL AND signing_secret_ciphertext IS NOT NULL AND events ? $2")
            .bind(tenant_id).bind(event).bind(encryption_key).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(WebhookDelivery {
                    url: row.try_get("url")?,
                    signing_secret: row.try_get("signing_secret")?,
                })
            })
            .collect()
    }

    pub async fn create_job(&self, tenant_id: Uuid, job: &JobMetadata) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO jobs (id, tenant_id, source_lang, target_langs, pipeline_mode, input_bucket, input_key, scene_ranges) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        ).bind(job.job_id.0).bind(tenant_id).bind(&job.source_lang)
         .bind(serde_json::to_value(&job.target_langs).expect("languages serialize"))
         .bind(job.pipeline_mode.to_string())
         .bind(job.s3_bucket.as_deref().expect("validated source bucket"))
         .bind(job.s3_key.as_deref().expect("validated source key"))
         .bind(serde_json::to_value(&job.scene_ranges).expect("scene ranges serialize"))
         .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn record_event(
        &self,
        job_id: JobId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO job_events (job_id, event_type, payload) VALUES ($1, $2, $3)")
            .bind(job_id.0)
            .bind(event_type)
            .bind(payload)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn events(
        &self,
        tenant_id: Uuid,
        job_id: JobId,
    ) -> Result<Vec<StoredJobEvent>, sqlx::Error> {
        let rows = sqlx::query("SELECT e.event_type, e.payload, e.created_at FROM job_events e JOIN jobs j ON j.id = e.job_id WHERE e.job_id = $1 AND j.tenant_id = $2 ORDER BY e.created_at")
            .bind(job_id.0).bind(tenant_id).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredJobEvent {
                    event_type: row.try_get("event_type")?,
                    payload: row.try_get("payload")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    pub async fn get_job(
        &self,
        tenant_id: Uuid,
        job_id: JobId,
    ) -> Result<Option<JobMetadata>, sqlx::Error> {
        sqlx::query("SELECT * FROM jobs WHERE id = $1 AND tenant_id = $2")
            .bind(job_id.0)
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await?
            .map(row_to_job)
            .transpose()
    }

    pub async fn list_jobs(&self, tenant_id: Uuid) -> Result<Vec<JobMetadata>, sqlx::Error> {
        let rows = sqlx::query("SELECT * FROM jobs WHERE tenant_id = $1 ORDER BY created_at DESC")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(row_to_job).collect()
    }

    pub async fn claim_next(
        &self,
        worker_id: &str,
        lease_seconds: i64,
    ) -> Result<Option<ClaimedJob>, sqlx::Error> {
        let row = sqlx::query(
            "UPDATE jobs SET status = 'running', worker_id = $1, attempt_count = attempt_count + 1, \
             started_at = COALESCE(started_at, now()), heartbeat_at = now(), \
             lease_expires_at = now() + make_interval(secs => $2), updated_at = now() \
             WHERE id = (SELECT id FROM jobs WHERE status = 'pending' ORDER BY created_at \
                         FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING *"
        ).bind(worker_id).bind(lease_seconds).fetch_optional(&self.pool).await?;
        row.map(|row| {
            let tenant_id = row.try_get("tenant_id")?;
            Ok(ClaimedJob {
                tenant_id,
                job: row_to_job(row)?,
            })
        })
        .transpose()
    }

    pub async fn heartbeat(
        &self,
        job_id: JobId,
        worker_id: &str,
        lease_seconds: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE jobs SET heartbeat_at = now(), lease_expires_at = now() + make_interval(secs => $3), updated_at = now() \
             WHERE id = $1 AND status = 'running' AND worker_id = $2"
        ).bind(job_id.0).bind(worker_id).bind(lease_seconds).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn recover_expired_leases(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE jobs SET status = CASE WHEN attempt_count >= max_attempts THEN 'failed' ELSE 'pending' END, \
             error_message = CASE WHEN attempt_count >= max_attempts THEN 'worker lease expired after maximum attempts' ELSE error_message END, \
             worker_id = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = now() \
             WHERE status = 'running' AND lease_expires_at < now()"
        ).execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn complete(
        &self,
        job_id: JobId,
        worker_id: &str,
        outputs: &[String],
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE jobs SET status = 'completed', output_files = $3, completed_at = now(), updated_at = now(), \
             lease_expires_at = NULL WHERE id = $1 AND status = 'running' AND worker_id = $2"
        ).bind(job_id.0).bind(worker_id).bind(serde_json::to_value(outputs).expect("outputs serialize"))
         .execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn fail(
        &self,
        job_id: JobId,
        worker_id: &str,
        message: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE jobs SET status = CASE WHEN attempt_count >= max_attempts THEN 'failed' ELSE 'pending' END, \
             error_message = $3, worker_id = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = now() \
             WHERE id = $1 AND status = 'running' AND worker_id = $2"
        ).bind(job_id.0).bind(worker_id).bind(message).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    /// Cancels a tenant-owned pending or running job. Clearing its lease makes
    /// any active worker lose ownership on its next heartbeat/complete update.
    pub async fn cancel(&self, tenant_id: Uuid, job_id: JobId) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE jobs SET status = 'cancelled', worker_id = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = now(), completed_at = now(), error_message = 'cancelled by user' WHERE id = $1 AND tenant_id = $2 AND status IN ('pending', 'running')")
            .bind(job_id.0).bind(tenant_id).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn replace_transcript(
        &self,
        job_id: JobId,
        language: Option<&str>,
        segments: &[TranscriptSegment],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM transcript_segments WHERE job_id = $1")
            .bind(job_id.0)
            .execute(&mut *tx)
            .await?;
        for segment in segments {
            sqlx::query("INSERT INTO transcript_segments (job_id, chunk_start_ms, start_ms, end_ms, language, text) VALUES ($1, 0, $2, $3, $4, $5)")
                .bind(job_id.0).bind(segment.start_ms as i64).bind(segment.end_ms as i64)
                .bind(language).bind(&segment.text).execute(&mut *tx).await?;
        }
        tx.commit().await
    }

    pub async fn start_stage(&self, job_id: JobId, stage: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO job_stages (job_id, stage, status, started_at, completed_at, detail) VALUES ($1, $2, 'running', now(), NULL, NULL) ON CONFLICT (job_id, stage) DO UPDATE SET status = 'running', started_at = now(), completed_at = NULL, detail = NULL")
            .bind(job_id.0).bind(stage).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn finish_stage(
        &self,
        job_id: JobId,
        stage: &str,
        detail: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE job_stages SET status = 'completed', completed_at = now(), detail = $3 WHERE job_id = $1 AND stage = $2")
            .bind(job_id.0).bind(stage).bind(detail).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn fail_active_stages(&self, job_id: JobId, detail: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE job_stages SET status = 'failed', completed_at = now(), detail = $2 WHERE job_id = $1 AND status = 'running'")
            .bind(job_id.0).bind(detail).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn stages(
        &self,
        tenant_id: Uuid,
        job_id: JobId,
    ) -> Result<Vec<StoredJobStage>, sqlx::Error> {
        let rows = sqlx::query("SELECT s.stage, s.status, s.detail, s.started_at, s.completed_at FROM job_stages s JOIN jobs j ON j.id = s.job_id WHERE s.job_id = $1 AND j.tenant_id = $2 ORDER BY s.started_at")
            .bind(job_id.0).bind(tenant_id).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredJobStage {
                    stage: row.try_get("stage")?,
                    status: row.try_get("status")?,
                    detail: row.try_get("detail")?,
                    started_at: row.try_get("started_at")?,
                    completed_at: row.try_get("completed_at")?,
                })
            })
            .collect()
    }

    pub async fn transcript(
        &self,
        tenant_id: Uuid,
        job_id: JobId,
    ) -> Result<Vec<StoredTranscriptSegment>, sqlx::Error> {
        let rows = sqlx::query("SELECT s.start_ms, s.end_ms, s.language, s.text FROM transcript_segments s JOIN jobs j ON j.id = s.job_id WHERE s.job_id = $1 AND j.tenant_id = $2 ORDER BY s.start_ms")
            .bind(job_id.0).bind(tenant_id).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredTranscriptSegment {
                    start_ms: row.try_get::<i64, _>("start_ms")? as u64,
                    end_ms: row.try_get::<i64, _>("end_ms")? as u64,
                    language: row.try_get("language")?,
                    text: row.try_get("text")?,
                })
            })
            .collect()
    }

    pub async fn replace_translation(
        &self,
        job_id: JobId,
        target: &str,
        segments: &[crate::models::TranscriptSegmentResponse],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM translation_segments WHERE job_id = $1 AND target_language = $2")
            .bind(job_id.0)
            .bind(target)
            .execute(&mut *tx)
            .await?;
        for segment in segments {
            sqlx::query("INSERT INTO translation_segments (job_id, target_language, start_ms, end_ms, text) VALUES ($1, $2, $3, $4, $5)")
                .bind(job_id.0).bind(target).bind(segment.start_ms as i64).bind(segment.end_ms as i64).bind(&segment.text).execute(&mut *tx).await?;
        }
        tx.commit().await
    }

    pub async fn replace_audio_segment(
        &self,
        job_id: JobId,
        target: &str,
        start_ms: u64,
        end_ms: u64,
        artifact_uri: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO audio_segments (job_id, target_language, start_ms, end_ms, artifact_uri) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (job_id, target_language, start_ms, end_ms) DO UPDATE SET artifact_uri = EXCLUDED.artifact_uri")
            .bind(job_id.0).bind(target).bind(start_ms as i64).bind(end_ms as i64).bind(artifact_uri).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn audio_segments_for_target(
        &self,
        job_id: JobId,
        target: &str,
    ) -> Result<Vec<StoredAudioSegment>, sqlx::Error> {
        let rows = sqlx::query("SELECT start_ms, artifact_uri FROM audio_segments WHERE job_id = $1 AND target_language = $2 ORDER BY start_ms")
            .bind(job_id.0).bind(target).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredAudioSegment {
                    start_ms: row.try_get::<i64, _>("start_ms")? as u64,
                    artifact_uri: row.try_get("artifact_uri")?,
                })
            })
            .collect()
    }
}

fn row_to_job(row: sqlx::postgres::PgRow) -> Result<JobMetadata, sqlx::Error> {
    let status: String = row.try_get("status")?;
    let mode: String = row.try_get("pipeline_mode")?;
    let targets: serde_json::Value = row.try_get("target_langs")?;
    let output_files: serde_json::Value = row.try_get("output_files")?;
    let scene_ranges: serde_json::Value = row.try_get("scene_ranges")?;
    let target_langs =
        serde_json::from_value(targets).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    let output_files = serde_json::from_value(output_files)
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    let scene_ranges = serde_json::from_value(scene_ranges)
        .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    Ok(JobMetadata {
        job_id: JobId(row.try_get::<Uuid, _>("id")?),
        source_lang: row.try_get("source_lang")?,
        target_langs,
        status: match status.as_str() {
            "pending" => JobStatus::Pending,
            "running" => JobStatus::Running,
            "completed" => JobStatus::Completed,
            "cancelled" => JobStatus::Cancelled,
            _ => JobStatus::Failed(
                row.try_get::<Option<String>, _>("error_message")?
                    .unwrap_or_else(|| "job failed".to_string()),
            ),
        },
        pipeline_mode: match mode.as_str() {
            "studio_batch" => PipelineMode::StudioBatch,
            "live_stream" => PipelineMode::LiveStream,
            _ => PipelineMode::CreatorSelective,
        },
        created_at: row.try_get::<DateTime<Utc>, _>("created_at")?,
        updated_at: row.try_get::<DateTime<Utc>, _>("updated_at")?,
        s3_bucket: Some(row.try_get("input_bucket")?),
        s3_key: Some(row.try_get("input_key")?),
        output_files,
        scene_ranges,
        error_message: row.try_get("error_message")?,
    })
}

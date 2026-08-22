use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::{error, info};
use uuid::Uuid;

use crate::models::{ErrorResponse, RegisterWebhookRequest, RegisterWebhookResponse};
use crate::state::AppState;
use nstream_core::{JobMetadata, NStreamError};

/// POST /webhooks/register
/// Register a new webhook URL to receive job status notifications.
pub async fn register_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterWebhookRequest>,
) -> Result<(StatusCode, Json<RegisterWebhookResponse>), (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = crate::auth::tenant_from_headers(&state, &headers).await?;
    let encryption_key = state.webhook_encryption_key.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "webhook_encryption_unavailable".into(),
                message: "WEBHOOK_ENCRYPTION_KEY is not configured".into(),
            }),
        )
    })?;
    let parsed_url = reqwest::Url::parse(&req.url);
    let insecure_allowed =
        std::env::var("NSTREAM_ALLOW_INSECURE_WEBHOOKS").as_deref() == Ok("true");
    if req.url.trim().is_empty()
        || parsed_url
            .as_ref()
            .ok()
            .and_then(reqwest::Url::host_str)
            .is_none()
        || (!parsed_url
            .as_ref()
            .map(|url| url.scheme() == "https")
            .unwrap_or(false)
            && !insecure_allowed)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_error".to_string(),
                message: "url must be an absolute HTTPS URL".to_string(),
            }),
        ));
    }
    if parsed_url
        .as_ref()
        .ok()
        .and_then(reqwest::Url::host_str)
        .is_some_and(is_private_ip_literal)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_error".into(),
                message: "webhook URL must not target a private IP address".into(),
            }),
        ));
    }

    let signing_secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let events = req
        .events
        .unwrap_or_else(|| vec!["completed".to_string(), "failed".to_string()]);

    let id = state
        .store
        .register_webhook(
            tenant_id,
            &req.url,
            &signing_secret,
            encryption_key,
            &events,
        )
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

    info!(webhook_id = %id, url = %req.url, "Webhook registered");

    Ok((
        StatusCode::CREATED,
        Json(RegisterWebhookResponse {
            id: id.to_string(),
            url: req.url,
            signing_secret,
            registered_at: Utc::now(),
        }),
    ))
}

/// Dispatch a job-status notification to all registered webhooks.
/// Fires-and-forgets each HTTP POST in a Tokio task.
#[allow(dead_code)]
pub async fn dispatch_webhooks(
    state: &AppState,
    tenant_id: Uuid,
    job: &JobMetadata,
    event: &str,
) -> Result<(), NStreamError> {
    let webhooks = state
        .store
        .webhooks_for_event(
            tenant_id,
            event,
            state.webhook_encryption_key.as_deref().ok_or_else(|| {
                NStreamError::Storage("WEBHOOK_ENCRYPTION_KEY is not configured".into())
            })?,
        )
        .await
        .map_err(|e| NStreamError::Storage(format!("could not load webhooks: {e}")))?;

    if webhooks.is_empty() {
        return Ok(());
    }

    let payload = serde_json::json!({
        "event": event,
        "job_id": job.job_id,
        "status": job.status,
        "updated_at": job.updated_at,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| NStreamError::Pipeline(format!("failed to build webhook client: {e}")))?;

    let payload_bytes = serde_json::to_vec(&payload)?;
    for webhook in webhooks {
        let url = webhook.url;
        let secret = webhook.signing_secret;
        let payload_clone = payload.clone();
        let client = client.clone();
        let signature = webhook_signature(&secret, &payload_bytes)?;
        tokio::spawn(async move {
            match client
                .post(&url)
                .json(&payload_clone)
                .header("X-NStream-Signature", format!("sha256={signature}"))
                .send()
                .await
            {
                Ok(resp) => {
                    info!(url = %url, status = %resp.status(), "Webhook dispatched");
                }
                Err(e) => {
                    error!(url = %url, error = %e, "Webhook dispatch failed");
                }
            }
        });
    }

    Ok(())
}

fn webhook_signature(secret: &str, payload: &[u8]) -> Result<String, NStreamError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| NStreamError::Pipeline(format!("invalid webhook secret: {e}")))?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn is_private_ip_literal(host: &str) -> bool {
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified()
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local()
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_private_ip_literal;

    #[test]
    fn rejects_private_ip_destinations() {
        assert!(is_private_ip_literal("127.0.0.1"));
        assert!(is_private_ip_literal("10.0.0.8"));
        assert!(is_private_ip_literal("::1"));
        assert!(!is_private_ip_literal("api.example.com"));
        assert!(!is_private_ip_literal("8.8.8.8"));
    }
}

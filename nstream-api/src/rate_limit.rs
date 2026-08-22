use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

pub struct RateLimiter {
    limit: u32,
    buckets: Mutex<HashMap<Vec<u8>, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn from_env() -> Self {
        let limit = std::env::var("NSTREAM_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value: &u32| *value > 0)
            .unwrap_or(60);
        Self {
            limit,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn allow(&self, identity: Vec<u8>) -> bool {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().expect("rate limiter mutex poisoned");
        buckets.retain(|_, (started, _)| now.duration_since(*started) < Duration::from_secs(120));
        let entry = buckets.entry(identity).or_insert((now, 0));
        if now.duration_since(entry.0) >= Duration::from_secs(60) {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.limit
    }
}

pub async fn enforce(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let identity = request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(crate::auth::key_hash)
        .unwrap_or_else(|| b"anonymous".to_vec());
    if state.rate_limiter.allow(identity) {
        next.run(request).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "60")],
            "rate limit exceeded",
        )
            .into_response()
    }
}

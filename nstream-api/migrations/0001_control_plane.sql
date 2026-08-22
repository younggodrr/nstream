CREATE TABLE tenants (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    source_lang TEXT NOT NULL,
    target_langs JSONB NOT NULL,
    pipeline_mode TEXT NOT NULL CHECK (pipeline_mode IN ('studio_batch', 'live_stream', 'creator_selective')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed')) DEFAULT 'pending',
    input_bucket TEXT NOT NULL,
    input_key TEXT NOT NULL,
    output_files JSONB NOT NULL DEFAULT '[]'::jsonb,
    error_message TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL DEFAULT 3 CHECK (max_attempts > 0),
    worker_id TEXT,
    lease_expires_at TIMESTAMPTZ,
    heartbeat_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

CREATE INDEX jobs_claim_idx ON jobs (created_at)
    WHERE status = 'pending';
CREATE INDEX jobs_lease_recovery_idx ON jobs (lease_expires_at)
    WHERE status = 'running';
CREATE INDEX jobs_tenant_created_idx ON jobs (tenant_id, created_at DESC);

CREATE TABLE webhook_subscriptions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id),
    url TEXT NOT NULL,
    signing_secret TEXT NOT NULL,
    events JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    disabled_at TIMESTAMPTZ
);

CREATE TABLE job_events (
    id BIGSERIAL PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

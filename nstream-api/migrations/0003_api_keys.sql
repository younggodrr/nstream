CREATE TABLE api_keys (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    key_prefix TEXT NOT NULL,
    key_hash BYTEA NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX api_keys_prefix_idx ON api_keys (key_prefix) WHERE revoked_at IS NULL;

CREATE TABLE audio_segments (
    id BIGSERIAL PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    target_language TEXT NOT NULL,
    start_ms BIGINT NOT NULL,
    end_ms BIGINT NOT NULL,
    artifact_uri TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (job_id, target_language, start_ms, end_ms)
);
CREATE INDEX audio_segments_job_language_idx ON audio_segments (job_id, target_language, start_ms);

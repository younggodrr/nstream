CREATE TABLE transcript_segments (
    id BIGSERIAL PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    chunk_start_ms BIGINT NOT NULL CHECK (chunk_start_ms >= 0),
    start_ms BIGINT NOT NULL CHECK (start_ms >= 0),
    end_ms BIGINT NOT NULL CHECK (end_ms >= start_ms),
    language TEXT,
    text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (job_id, start_ms, end_ms, text)
);

CREATE INDEX transcript_segments_job_time_idx ON transcript_segments (job_id, start_ms);

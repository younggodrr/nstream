CREATE TABLE job_stages (
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    stage TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed')),
    detail TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    PRIMARY KEY (job_id, stage)
);

CREATE INDEX job_stages_job_id_started_at_idx ON job_stages (job_id, started_at);

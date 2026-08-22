ALTER TABLE jobs DROP CONSTRAINT jobs_status_check;
ALTER TABLE jobs ADD CONSTRAINT jobs_status_check
    CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled'));

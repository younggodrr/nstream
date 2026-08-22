use std::collections::HashMap;
use tracing::{debug, info};

use nstream_core::JobId;

/// Tracks GPU compute seconds consumed per job and computes estimated costs.
#[derive(Debug, Default)]
pub struct CostTracker {
    /// Map of job_id → total GPU seconds consumed.
    gpu_seconds: HashMap<JobId, f64>,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            gpu_seconds: HashMap::new(),
        }
    }

    /// Record additional GPU compute seconds for the given job.
    pub fn record(&mut self, job_id: JobId, seconds: f64) {
        let entry = self.gpu_seconds.entry(job_id).or_insert(0.0);
        *entry += seconds;
        debug!(
            job_id = %job_id,
            added_seconds = seconds,
            total_seconds = *entry,
            "Recorded GPU time"
        );
    }

    /// Compute the total estimated cost for a job at the given rate.
    ///
    /// # Arguments
    /// * `job_id` — the job to query
    /// * `rate_per_second` — cost in USD per GPU-second (e.g. 0.0001)
    ///
    /// Returns `0.0` if the job has no recorded GPU time.
    pub fn total_cost(&self, job_id: JobId, rate_per_second: f64) -> f64 {
        let seconds = self.gpu_seconds.get(&job_id).copied().unwrap_or(0.0);
        let cost = seconds * rate_per_second;
        info!(
            job_id = %job_id,
            gpu_seconds = seconds,
            rate_per_second = rate_per_second,
            estimated_cost_usd = cost,
            "Cost computed"
        );
        cost
    }

    /// Returns the total GPU seconds recorded for a job.
    pub fn gpu_seconds(&self, job_id: JobId) -> f64 {
        self.gpu_seconds.get(&job_id).copied().unwrap_or(0.0)
    }

    /// Returns total GPU seconds across all jobs.
    pub fn total_gpu_seconds(&self) -> f64 {
        self.gpu_seconds.values().sum()
    }

    /// Remove tracking data for a job (e.g. after archiving).
    pub fn clear_job(&mut self, job_id: &JobId) {
        self.gpu_seconds.remove(job_id);
    }

    /// Returns a snapshot of all (job_id, gpu_seconds) pairs.
    pub fn all_entries(&self) -> Vec<(JobId, f64)> {
        self.gpu_seconds.iter().map(|(k, v)| (*k, *v)).collect()
    }
}

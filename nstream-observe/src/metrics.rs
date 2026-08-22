use prometheus::{Counter, Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry};
use tracing::info;

/// All Prometheus metrics exported by NStream.
#[allow(dead_code)]
pub struct NStreamMetrics {
    /// Total number of jobs submitted via the API.
    pub jobs_submitted: IntCounter,
    /// Total number of jobs that completed successfully.
    pub jobs_completed: IntCounter,
    /// Total number of jobs that failed.
    pub jobs_failed: IntCounter,
    /// Histogram of end-to-end job processing duration in seconds.
    pub job_duration_seconds: Histogram,
    /// Current number of GPU inference workers that are actively processing.
    pub active_gpu_workers: IntGauge,
    /// Total GPU compute seconds consumed across all jobs.
    pub gpu_seconds_total: Counter,
}

impl NStreamMetrics {
    /// Register all metrics against the given Prometheus `Registry`.
    pub fn register(registry: &Registry) -> Self {
        let jobs_submitted = IntCounter::with_opts(Opts::new(
            "nstream_jobs_submitted_total",
            "Total number of jobs submitted",
        ))
        .expect("metric creation failed");

        let jobs_completed = IntCounter::with_opts(Opts::new(
            "nstream_jobs_completed_total",
            "Total number of jobs completed successfully",
        ))
        .expect("metric creation failed");

        let jobs_failed = IntCounter::with_opts(Opts::new(
            "nstream_jobs_failed_total",
            "Total number of jobs that failed",
        ))
        .expect("metric creation failed");

        let job_duration_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "nstream_job_duration_seconds",
                "End-to-end job processing duration in seconds",
            )
            .buckets(vec![
                0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0,
            ]),
        )
        .expect("metric creation failed");

        let active_gpu_workers = IntGauge::with_opts(Opts::new(
            "nstream_active_gpu_workers",
            "Number of currently active GPU inference workers",
        ))
        .expect("metric creation failed");

        let gpu_seconds_total = Counter::with_opts(Opts::new(
            "nstream_gpu_seconds_total",
            "Total GPU compute seconds consumed",
        ))
        .expect("metric creation failed");

        registry
            .register(Box::new(jobs_submitted.clone()))
            .expect("registration failed");
        registry
            .register(Box::new(jobs_completed.clone()))
            .expect("registration failed");
        registry
            .register(Box::new(jobs_failed.clone()))
            .expect("registration failed");
        registry
            .register(Box::new(job_duration_seconds.clone()))
            .expect("registration failed");
        registry
            .register(Box::new(active_gpu_workers.clone()))
            .expect("registration failed");
        registry
            .register(Box::new(gpu_seconds_total.clone()))
            .expect("registration failed");

        info!("Prometheus metrics registered");

        Self {
            jobs_submitted,
            jobs_completed,
            jobs_failed,
            job_duration_seconds,
            active_gpu_workers,
            gpu_seconds_total,
        }
    }

    /// Convenience: increment jobs_submitted and return a timer handle.
    pub fn record_job_submitted(&self) {
        self.jobs_submitted.inc();
    }

    /// Convenience: increment jobs_completed and observe duration.
    pub fn record_job_completed(&self, duration_secs: f64) {
        self.jobs_completed.inc();
        self.job_duration_seconds.observe(duration_secs);
    }

    /// Convenience: increment jobs_failed.
    pub fn record_job_failed(&self) {
        self.jobs_failed.inc();
    }

    /// Convenience: update active GPU worker gauge.
    pub fn set_active_gpu_workers(&self, count: i64) {
        self.active_gpu_workers.set(count);
    }

    /// Convenience: add GPU compute seconds.
    pub fn add_gpu_seconds(&self, seconds: f64) {
        self.gpu_seconds_total.inc_by(seconds);
    }
}

/// Create a new default Prometheus registry and register all NStream metrics.
pub fn register() -> (Registry, NStreamMetrics) {
    let registry = Registry::new();
    let metrics = NStreamMetrics::register(&registry);
    (registry, metrics)
}

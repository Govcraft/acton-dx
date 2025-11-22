// ! Job observability with OpenTelemetry and structured logging.

use super::{JobId, JobStatus};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::{debug, error, info, warn};

/// Job execution context for logging and tracing.
#[derive(Debug, Clone, Serialize)]
pub struct JobExecutionContext {
    /// Job ID.
    pub job_id: JobId,
    /// Job type name.
    pub job_type: String,
    /// When the job started executing.
    pub started_at: DateTime<Utc>,
    /// Job priority.
    pub priority: i32,
    /// Current attempt number (0-based).
    pub attempt: u32,
    /// Maximum retries allowed.
    pub max_retries: u32,
}

impl JobExecutionContext {
    /// Create a new job execution context.
    #[must_use]
    pub fn new(
        job_id: JobId,
        job_type: String,
        priority: i32,
        attempt: u32,
        max_retries: u32,
    ) -> Self {
        Self {
            job_id,
            job_type,
            started_at: Utc::now(),
            priority,
            attempt,
            max_retries,
        }
    }

    /// Calculate execution duration from start time.
    #[must_use]
    pub fn execution_duration_ms(&self) -> u64 {
        Utc::now()
            .signed_duration_since(self.started_at)
            .num_milliseconds()
            .max(0)
            .try_into()
            .unwrap_or(0)
    }

    /// Log job start.
    pub fn log_start(&self) {
        info!(
            job_id = %self.job_id,
            job_type = %self.job_type,
            priority = self.priority,
            attempt = self.attempt,
            max_retries = self.max_retries,
            "Job execution started"
        );
    }

    /// Log job completion.
    pub fn log_completion(&self) {
        let duration_ms = self.execution_duration_ms();
        info!(
            job_id = %self.job_id,
            job_type = %self.job_type,
            duration_ms = duration_ms,
            attempt = self.attempt,
            "Job completed successfully"
        );
    }

    /// Log job failure.
    pub fn log_failure(&self, error: &str) {
        let duration_ms = self.execution_duration_ms();
        error!(
            job_id = %self.job_id,
            job_type = %self.job_type,
            duration_ms = duration_ms,
            attempt = self.attempt,
            max_retries = self.max_retries,
            error = error,
            "Job execution failed"
        );
    }

    /// Log job retry.
    pub fn log_retry(&self, retry_after_ms: u64, error: &str) {
        warn!(
            job_id = %self.job_id,
            job_type = %self.job_type,
            attempt = self.attempt,
            max_retries = self.max_retries,
            retry_after_ms = retry_after_ms,
            error = error,
            "Job failed, will retry"
        );
    }

    /// Log job cancellation.
    pub fn log_cancellation(&self) {
        let duration_ms = self.execution_duration_ms();
        warn!(
            job_id = %self.job_id,
            job_type = %self.job_type,
            duration_ms = duration_ms,
            "Job cancelled"
        );
    }
}

/// Job queue observability metrics logger.
pub struct JobQueueObserver;

impl JobQueueObserver {
    /// Log job enqueued.
    pub fn log_enqueued(job_id: JobId, job_type: &str, priority: i32) {
        debug!(
            job_id = %job_id,
            job_type = job_type,
            priority = priority,
            "Job enqueued"
        );
    }

    /// Log job dequeued for processing.
    pub fn log_dequeued(job_id: JobId, job_type: &str, queue_time_ms: u64) {
        debug!(
            job_id = %job_id,
            job_type = job_type,
            queue_time_ms = queue_time_ms,
            "Job dequeued for processing"
        );
    }

    /// Log queue full rejection.
    pub fn log_queue_full(job_id: JobId, job_type: &str, max_size: usize) {
        warn!(
            job_id = %job_id,
            job_type = job_type,
            max_queue_size = max_size,
            "Job rejected: queue is full"
        );
    }

    /// Log dead letter queue movement.
    pub fn log_moved_to_dlq(
        job_id: JobId,
        job_type: &str,
        attempts: u32,
        final_error: &str,
    ) {
        error!(
            job_id = %job_id,
            job_type = job_type,
            attempts = attempts,
            error = final_error,
            "Job moved to dead letter queue after exhausting retries"
        );
    }

    /// Log job status change.
    pub fn log_status_change(job_id: JobId, job_type: &str, old_status: &JobStatus, new_status: &JobStatus) {
        debug!(
            job_id = %job_id,
            job_type = job_type,
            old_status = old_status.name(),
            new_status = new_status.name(),
            "Job status changed"
        );
    }
}

/// Performance metrics recorder for jobs.
pub struct JobPerformanceRecorder {
    /// Job type for aggregation.
    pub job_type: String,
}

impl JobPerformanceRecorder {
    /// Create a new performance recorder for a job type.
    #[must_use]
    pub const fn new(job_type: String) -> Self {
        Self { job_type }
    }

    /// Record execution time for a job.
    pub fn record_execution_time(&self, job_id: JobId, duration_ms: u64, success: bool) {
        debug!(
            job_id = %job_id,
            job_type = %self.job_type,
            duration_ms = duration_ms,
            success = success,
            "Job execution completed"
        );

        // In production, this would emit metrics to OpenTelemetry/Prometheus
        // For now, we just log it
    }

    /// Record queue wait time (time from enqueue to start).
    pub fn record_queue_wait_time(&self, job_id: JobId, wait_time_ms: u64) {
        debug!(
            job_id = %job_id,
            job_type = %self.job_type,
            queue_wait_ms = wait_time_ms,
            "Job queue wait time"
        );
    }

    /// Record retry attempt.
    pub fn record_retry(&self, job_id: JobId, attempt: u32) {
        debug!(
            job_id = %job_id,
            job_type = %self.job_type,
            attempt = attempt,
            "Job retry attempt"
        );
    }
}

#[cfg(feature = "otel-metrics")]
mod otel {
    use super::*;
    use opentelemetry::metrics::{Counter, Histogram, Meter};
    use std::sync::Arc;

    /// OpenTelemetry metrics for job processing.
    pub struct JobMetricsCollector {
        /// Job execution counter.
        jobs_executed: Counter<u64>,
        /// Job failure counter.
        jobs_failed: Counter<u64>,
        /// Job execution duration histogram.
        execution_duration: Histogram<u64>,
        /// Queue wait time histogram.
        queue_wait_time: Histogram<u64>,
    }

    impl JobMetricsCollector {
        /// Create a new metrics collector.
        ///
        /// # Errors
        ///
        /// Returns error if metric creation fails.
        pub fn new(meter: &Meter) -> Result<Arc<Self>, opentelemetry::metrics::MetricsError> {
            Ok(Arc::new(Self {
                jobs_executed: meter
                    .u64_counter("acton_htmx.jobs.executed")
                    .with_description("Number of jobs executed")
                    .build()?,
                jobs_failed: meter
                    .u64_counter("acton_htmx.jobs.failed")
                    .with_description("Number of jobs that failed")
                    .build()?,
                execution_duration: meter
                    .u64_histogram("acton_htmx.jobs.execution_duration_ms")
                    .with_description("Job execution duration in milliseconds")
                    .build()?,
                queue_wait_time: meter
                    .u64_histogram("acton_htmx.jobs.queue_wait_time_ms")
                    .with_description("Time jobs spend waiting in queue (milliseconds)")
                    .build()?,
            }))
        }

        /// Record a job execution.
        pub fn record_execution(&self, job_type: &str, duration_ms: u64, success: bool) {
            let attributes = &[
                opentelemetry::KeyValue::new("job_type", job_type.to_string()),
                opentelemetry::KeyValue::new("success", success),
            ];

            if success {
                self.jobs_executed.add(1, attributes);
            } else {
                self.jobs_failed.add(1, attributes);
            }

            self.execution_duration.record(duration_ms, attributes);
        }

        /// Record queue wait time.
        pub fn record_queue_wait(&self, job_type: &str, wait_time_ms: u64) {
            let attributes = &[opentelemetry::KeyValue::new("job_type", job_type.to_string())];
            self.queue_wait_time.record(wait_time_ms, attributes);
        }
    }
}

#[cfg(feature = "otel-metrics")]
pub use otel::JobMetricsCollector;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_execution_context_creation() {
        let ctx = JobExecutionContext::new(
            JobId::new(),
            "TestJob".to_string(),
            5,
            0,
            3,
        );

        assert_eq!(ctx.job_type, "TestJob");
        assert_eq!(ctx.priority, 5);
        assert_eq!(ctx.attempt, 0);
        assert_eq!(ctx.max_retries, 3);
    }

    #[test]
    fn test_execution_duration() {
        let ctx = JobExecutionContext::new(
            JobId::new(),
            "TestJob".to_string(),
            0,
            0,
            3,
        );

        // Duration should be very small (< 100ms)
        let duration = ctx.execution_duration_ms();
        assert!(duration < 100);
    }

    #[test]
    fn test_performance_recorder() {
        let recorder = JobPerformanceRecorder::new("TestJob".to_string());
        assert_eq!(recorder.job_type, "TestJob");

        // These should not panic
        recorder.record_execution_time(JobId::new(), 100, true);
        recorder.record_queue_wait_time(JobId::new(), 50);
        recorder.record_retry(JobId::new(), 1);
    }
}

//! Messages for the job agent.

use crate::jobs::{JobId, JobStatus};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Enqueue a new job for processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueJob {
    /// Unique job identifier.
    pub id: JobId,
    /// Job type name.
    pub job_type: String,
    /// Serialized job payload.
    pub payload: Vec<u8>,
    /// Job priority (higher = more important).
    pub priority: i32,
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Job execution timeout.
    pub timeout: Duration,
}

/// Response to job enqueue request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnqueued {
    /// The enqueued job ID.
    pub id: JobId,
}

/// Get the status of a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetJobStatus {
    /// Job ID to query.
    pub id: JobId,
}

/// Response containing job status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatusResponse {
    /// Job ID.
    pub id: JobId,
    /// Current status (None if job not found).
    pub status: Option<JobStatus>,
}

/// Request job metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMetrics;

/// Job processing metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobMetrics {
    /// Total jobs enqueued.
    pub jobs_enqueued: u64,
    /// Total jobs dequeued.
    pub jobs_dequeued: u64,
    /// Total jobs completed successfully.
    pub jobs_completed: u64,
    /// Total jobs failed.
    pub jobs_failed: u64,
    /// Total jobs rejected (queue full).
    pub jobs_rejected: u64,
    /// Total jobs in dead letter queue.
    pub jobs_in_dlq: u64,
    /// Current queue size.
    pub current_queue_size: usize,
    /// Current number of running jobs.
    pub current_running: usize,
    /// Total execution time in milliseconds.
    pub total_execution_time_ms: u64,
    /// Average execution time in milliseconds.
    pub avg_execution_time_ms: u64,
    /// Minimum execution time in milliseconds.
    pub min_execution_time_ms: u64,
    /// Maximum execution time in milliseconds.
    pub max_execution_time_ms: u64,
    /// P50 (median) execution time in milliseconds.
    pub p50_execution_time_ms: u64,
    /// P95 execution time in milliseconds.
    pub p95_execution_time_ms: u64,
    /// P99 execution time in milliseconds.
    pub p99_execution_time_ms: u64,
}

impl JobMetrics {
    /// Update metrics with a completed job execution time.
    ///
    /// This updates percentile calculations using a simple streaming algorithm.
    /// For production use, consider using a histogram library like `hdrhistogram`.
    pub const fn record_execution_time(&mut self, execution_time_ms: u64) {
        self.total_execution_time_ms = self.total_execution_time_ms.saturating_add(execution_time_ms);

        // Update min/max
        if self.min_execution_time_ms == 0 || execution_time_ms < self.min_execution_time_ms {
            self.min_execution_time_ms = execution_time_ms;
        }
        if execution_time_ms > self.max_execution_time_ms {
            self.max_execution_time_ms = execution_time_ms;
        }

        // Update average
        if self.jobs_completed > 0 {
            self.avg_execution_time_ms = self.total_execution_time_ms / self.jobs_completed;
        }

        // Simple percentile estimation (will be replaced with histogram in production)
        // For now, use max as p99, avg as p50, and interpolate p95
        self.p50_execution_time_ms = self.avg_execution_time_ms;
        self.p95_execution_time_ms = self.avg_execution_time_ms +
            ((self.max_execution_time_ms.saturating_sub(self.avg_execution_time_ms)) * 75 / 100);
        self.p99_execution_time_ms = self.max_execution_time_ms;
    }

    /// Calculate failure rate as percentage (0-100).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Acceptable for metrics
    pub fn failure_rate(&self) -> f64 {
        let total = self.jobs_completed + self.jobs_failed;
        if total == 0 {
            0.0
        } else {
            (self.jobs_failed as f64 / total as f64) * 100.0
        }
    }
}

/// Internal message to trigger job processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Will be used in Week 5 for job processing loop
pub(super) struct ProcessJobs;

/// Internal message to cleanup expired jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Will be used in Week 5 for cleanup scheduling
pub(super) struct CleanupExpiredJobs;

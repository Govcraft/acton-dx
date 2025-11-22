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
}

/// Internal message to trigger job processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Will be used in Week 5 for job processing loop
pub(super) struct ProcessJobs;

/// Internal message to cleanup expired jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // Will be used in Week 5 for cleanup scheduling
pub(super) struct CleanupExpiredJobs;

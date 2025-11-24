//! Messages for the job agent.

use crate::jobs::{JobId, JobStatus};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};

/// Response channel type for web handler pattern.
///
/// Wraps `oneshot::Sender` in `Arc<Mutex<Option<T>>>` to satisfy
/// `Clone + Debug` requirements for acton-reactive messages.
pub type ResponseChannel<T> = Arc<Mutex<Option<oneshot::Sender<T>>>>;

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

/// Get the status of a job (agent-to-agent pattern).
///
/// **Deprecated**: Use [`GetJobStatusRequest`] for web handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetJobStatus {
    /// Job ID to query.
    pub id: JobId,
}

/// Response containing job status (agent-to-agent pattern).
///
/// **Deprecated**: Use [`GetJobStatusRequest`] for web handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatusResponse {
    /// Job ID.
    pub id: JobId,
    /// Current status (None if job not found).
    pub status: Option<JobStatus>,
}

/// Request job metrics (agent-to-agent pattern).
///
/// **Deprecated**: Use [`GetMetricsRequest`] for web handlers.
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

// ============================================================================
// Web Handler Pattern Messages (HTTP handler to agent communication)
// ============================================================================

/// Request job metrics (web handler pattern).
///
/// Used by HTTP handlers to query job statistics. Uses oneshot channel
/// for response to avoid blocking the handler.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::jobs::agent::messages::GetMetricsRequest;
/// use std::time::Duration;
///
/// async fn handler(State(state): State<ActonHtmxState>) -> Result<Response> {
///     let (request, rx) = GetMetricsRequest::new();
///     state.job_agent().send(request).await;
///
///     let timeout = Duration::from_millis(100);
///     let metrics = tokio::time::timeout(timeout, rx).await??;
///
///     Ok(Json(metrics).into_response())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct GetMetricsRequest {
    /// Response channel for metrics.
    pub response_tx: ResponseChannel<JobMetrics>,
}

impl GetMetricsRequest {
    /// Create a new metrics request with response channel.
    ///
    /// Returns a tuple of (request, receiver) where the request should be
    /// sent to the agent and the receiver awaited for the response.
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<JobMetrics>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            response_tx: Arc::new(Mutex::new(Some(tx))),
        };
        (request, rx)
    }
}

/// Request job status (web handler pattern).
///
/// Used by HTTP handlers to query the status of a specific job.
/// Uses oneshot channel for response to avoid blocking the handler.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::jobs::agent::messages::GetJobStatusRequest;
/// use std::time::Duration;
///
/// async fn handler(
///     State(state): State<ActonHtmxState>,
///     Path(job_id): Path<JobId>,
/// ) -> Result<Response> {
///     let (request, rx) = GetJobStatusRequest::new(job_id);
///     state.job_agent().send(request).await;
///
///     let timeout = Duration::from_millis(100);
///     let status = tokio::time::timeout(timeout, rx).await??;
///
///     Ok(Json(status).into_response())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct GetJobStatusRequest {
    /// Job ID to query.
    pub id: JobId,
    /// Response channel for status.
    pub response_tx: ResponseChannel<Option<JobStatus>>,
}

impl GetJobStatusRequest {
    /// Create a new job status request with response channel.
    ///
    /// Returns a tuple of (request, receiver) where the request should be
    /// sent to the agent and the receiver awaited for the response.
    #[must_use]
    pub fn new(id: JobId) -> (Self, oneshot::Receiver<Option<JobStatus>>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            id,
            response_tx: Arc::new(Mutex::new(Some(tx))),
        };
        (request, rx)
    }
}

/// Retry a failed job (web handler pattern).
///
/// Re-queues a job from the dead letter queue back into the main queue
/// for another execution attempt.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::jobs::agent::messages::RetryJobRequest;
///
/// async fn handler(
///     State(state): State<ActonHtmxState>,
///     Path(job_id): Path<JobId>,
/// ) -> Result<Response> {
///     let (request, rx) = RetryJobRequest::new(job_id);
///     state.job_agent().send(request).await;
///
///     let success = tokio::time::timeout(Duration::from_millis(100), rx).await??;
///     Ok(if success {
///         StatusCode::OK
///     } else {
///         StatusCode::NOT_FOUND
///     }.into_response())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct RetryJobRequest {
    /// Job ID to retry.
    pub id: JobId,
    /// Response channel indicating success.
    pub response_tx: ResponseChannel<bool>,
}

impl RetryJobRequest {
    /// Create a new retry job request with response channel.
    ///
    /// Returns a tuple of (request, receiver) where the request should be
    /// sent to the agent and the receiver awaited for the response.
    #[must_use]
    pub fn new(id: JobId) -> (Self, oneshot::Receiver<bool>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            id,
            response_tx: Arc::new(Mutex::new(Some(tx))),
        };
        (request, rx)
    }
}

/// Retry all failed jobs (web handler pattern).
///
/// Re-queues all jobs from the dead letter queue back into the main queue.
/// Returns the number of jobs successfully retried.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::jobs::agent::messages::RetryAllFailedRequest;
///
/// async fn handler(State(state): State<ActonHtmxState>) -> Result<Response> {
///     let (request, rx) = RetryAllFailedRequest::new();
///     state.job_agent().send(request).await;
///
///     let count = tokio::time::timeout(Duration::from_millis(500), rx).await??;
///     Ok(Json(json!({ "retried": count })).into_response())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct RetryAllFailedRequest {
    /// Response channel with count of retried jobs.
    pub response_tx: ResponseChannel<usize>,
}

impl RetryAllFailedRequest {
    /// Create a new retry all failed request with response channel.
    ///
    /// Returns a tuple of (request, receiver) where the request should be
    /// sent to the agent and the receiver awaited for the response.
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<usize>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            response_tx: Arc::new(Mutex::new(Some(tx))),
        };
        (request, rx)
    }
}

/// Cancel a running or pending job (web handler pattern).
///
/// Attempts to cancel a job. If the job is pending, it's removed from the queue.
/// If it's currently running, a cancellation signal is sent.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::jobs::agent::messages::CancelJobRequest;
///
/// async fn handler(
///     State(state): State<ActonHtmxState>,
///     Path(job_id): Path<JobId>,
/// ) -> Result<Response> {
///     let (request, rx) = CancelJobRequest::new(job_id);
///     state.job_agent().send(request).await;
///
///     let success = tokio::time::timeout(Duration::from_millis(100), rx).await??;
///     Ok(if success {
///         StatusCode::OK
///     } else {
///         StatusCode::NOT_FOUND
///     }.into_response())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct CancelJobRequest {
    /// Job ID to cancel.
    pub id: JobId,
    /// Response channel indicating success.
    pub response_tx: ResponseChannel<bool>,
}

impl CancelJobRequest {
    /// Create a new cancel job request with response channel.
    ///
    /// Returns a tuple of (request, receiver) where the request should be
    /// sent to the agent and the receiver awaited for the response.
    #[must_use]
    pub fn new(id: JobId) -> (Self, oneshot::Receiver<bool>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            id,
            response_tx: Arc::new(Mutex::new(Some(tx))),
        };
        (request, rx)
    }
}

/// Clear the dead letter queue (web handler pattern).
///
/// Permanently removes all jobs from the dead letter queue.
/// This operation cannot be undone.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::jobs::agent::messages::ClearDeadLetterQueueRequest;
///
/// async fn handler(State(state): State<ActonHtmxState>) -> Result<Response> {
///     let (request, rx) = ClearDeadLetterQueueRequest::new();
///     state.job_agent().send(request).await;
///
///     let count = tokio::time::timeout(Duration::from_millis(100), rx).await??;
///     Ok(Json(json!({ "cleared": count })).into_response())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct ClearDeadLetterQueueRequest {
    /// Response channel with count of cleared jobs.
    pub response_tx: ResponseChannel<usize>,
}

impl ClearDeadLetterQueueRequest {
    /// Create a new clear dead letter queue request with response channel.
    ///
    /// Returns a tuple of (request, receiver) where the request should be
    /// sent to the agent and the receiver awaited for the response.
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<usize>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            response_tx: Arc::new(Mutex::new(Some(tx))),
        };
        (request, rx)
    }
}

/// Job history page response containing records and pagination info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHistoryPage {
    /// Job history records for this page.
    pub jobs: Vec<super::history::JobHistoryRecord>,
    /// Current page number (1-indexed).
    pub page: usize,
    /// Number of records per page.
    pub page_size: usize,
    /// Total number of matching records across all pages.
    pub total_count: usize,
    /// Whether there is a previous page.
    pub has_prev: bool,
    /// Whether there is a next page.
    pub has_next: bool,
}

impl JobHistoryPage {
    /// Create a new job history page from records and pagination info.
    #[must_use]
    pub const fn new(
        jobs: Vec<super::history::JobHistoryRecord>,
        page: usize,
        page_size: usize,
        total_count: usize,
    ) -> Self {
        let has_prev = page > 1;
        let total_pages = total_count.div_ceil(page_size);
        let has_next = page < total_pages;

        Self {
            jobs,
            page,
            page_size,
            total_count,
            has_prev,
            has_next,
        }
    }

    /// Get the previous page number.
    #[must_use]
    pub fn prev_page(&self) -> usize {
        self.page.saturating_sub(1).max(1)
    }

    /// Get the next page number.
    #[must_use]
    pub const fn next_page(&self) -> usize {
        self.page + 1
    }

    /// Get the starting record number for this page (1-indexed).
    #[must_use]
    pub fn page_start(&self) -> usize {
        if self.jobs.is_empty() {
            0
        } else {
            (self.page - 1) * self.page_size + 1
        }
    }

    /// Get the ending record number for this page (1-indexed).
    #[must_use]
    pub fn page_end(&self) -> usize {
        if self.jobs.is_empty() {
            0
        } else {
            self.page_start() + self.jobs.len() - 1
        }
    }
}

/// Request job history with pagination and search (web handler pattern).
///
/// Retrieves completed job history with optional search filtering
/// and pagination support.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::jobs::agent::messages::GetJobHistoryRequest;
///
/// async fn handler(
///     State(state): State<ActonHtmxState>,
///     Query(params): Query<HistoryParams>,
/// ) -> Result<Response> {
///     let (request, rx) = GetJobHistoryRequest::new(
///         params.page.unwrap_or(1),
///         params.page_size.unwrap_or(20),
///         params.search,
///     );
///     state.job_agent().send(request).await;
///
///     let timeout = Duration::from_millis(200);
///     let history = tokio::time::timeout(timeout, rx).await??;
///
///     Ok(Json(history).into_response())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct GetJobHistoryRequest {
    /// Page number (1-indexed).
    pub page: usize,
    /// Number of records per page.
    pub page_size: usize,
    /// Optional search query to filter results.
    pub search_query: Option<String>,
    /// Response channel for history page.
    pub response_tx: ResponseChannel<JobHistoryPage>,
}

impl GetJobHistoryRequest {
    /// Create a new job history request with response channel.
    ///
    /// # Arguments
    ///
    /// * `page` - Page number (1-indexed)
    /// * `page_size` - Number of records per page
    /// * `search_query` - Optional search string to filter records
    ///
    /// Returns a tuple of (request, receiver) where the request should be
    /// sent to the agent and the receiver awaited for the response.
    #[must_use]
    pub fn new(
        page: usize,
        page_size: usize,
        search_query: Option<String>,
    ) -> (Self, oneshot::Receiver<JobHistoryPage>) {
        let (tx, rx) = oneshot::channel();
        let request = Self {
            page: page.max(1), // Ensure page is at least 1
            page_size: page_size.clamp(1, 100), // Clamp between 1-100
            search_query,
            response_tx: Arc::new(Mutex::new(Some(tx))),
        };
        (request, rx)
    }
}

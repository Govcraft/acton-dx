//! Job processing agent using acton-reactive.

pub mod history;
pub(crate) mod messages;
pub(crate) mod persistence;
pub(crate) mod queue;
#[cfg(feature = "redis")]
pub mod redis_agent;
pub mod scheduled;

pub use history::JobHistoryRecord;
pub use messages::{
    CancelJobRequest, ClearDeadLetterQueueRequest, EnqueueJob, GetJobHistoryRequest,
    GetJobStatusRequest, GetMetricsRequest, JobEnqueued, JobHistoryPage, JobMetrics,
    ResponseChannel, RetryAllFailedRequest, RetryJobRequest,
};
#[cfg(feature = "redis")]
pub use redis_agent::RedisPersistenceAgent;
pub use scheduled::{ScheduledJobAgent, ScheduledJobEntry, ScheduledJobMessage, ScheduledJobResponse, start_scheduler_loop};

use super::{JobContext, JobId, JobStatus};
use acton_reactive::prelude::*;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use history::JobHistory;
use messages::{GetJobStatus, GetMetrics, JobStatusResponse};
use queue::{JobQueue, QueuedJob};

// Type alias for the ManagedActor builder type
type JobActorBuilder = ManagedActor<Idle, JobAgent>;

/// Background job processing agent.
///
/// Manages a queue of background jobs with:
/// - Priority-based execution
/// - Redis persistence (via dedicated `RedisPersistenceAgent`)
/// - Automatic retry with exponential backoff
/// - Dead letter queue for failed jobs
/// - Job history tracking with pagination
/// - Graceful shutdown
/// - Service access via [`JobContext`](crate::jobs::JobContext)
#[derive(Clone)]
pub struct JobAgent {
    /// In-memory priority queue.
    queue: Arc<RwLock<JobQueue>>,
    /// Currently running jobs.
    running: Arc<RwLock<HashMap<JobId, JobStatus>>>,
    /// Dead letter queue for permanently failed jobs.
    dead_letter: Arc<RwLock<HashMap<JobId, QueuedJob>>>,
    /// Job history with completed jobs (bounded circular buffer).
    history: Arc<RwLock<JobHistory>>,
    /// Job metrics.
    metrics: Arc<RwLock<JobMetrics>>,
    /// Job execution context with services.
    ///
    /// Provides jobs with access to email sender, database pool, file storage, etc.
    context: Arc<JobContext>,
    /// Handle to Redis persistence actor (optional, for persistence).
    #[cfg(feature = "redis")]
    redis_persistence: Option<ActorHandle>,
}

impl std::fmt::Debug for JobAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("JobAgent");
        debug_struct
            .field("queue", &"<JobQueue>")
            .field("running", &self.running.read().len())
            .field("dead_letter", &self.dead_letter.read().len())
            .field("history", &self.history.read().len())
            .field("metrics", &self.metrics.read())
            .field("context", &self.context);

        #[cfg(feature = "redis")]
        debug_struct.field("redis_persistence", &self.redis_persistence.is_some());

        debug_struct.finish()
    }
}

impl Default for JobAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl JobAgent {
    /// Create a new job agent without Redis or services.
    ///
    /// Use [`with_context`](Self::with_context) to provide services.
    /// Use [`with_persistence`](Self::with_persistence) to enable Redis persistence.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(RwLock::new(JobQueue::new(10_000))),
            running: Arc::new(RwLock::new(HashMap::new())),
            dead_letter: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(JobHistory::new(1000))), // Keep last 1000 jobs
            metrics: Arc::new(RwLock::new(JobMetrics::default())),
            context: Arc::new(JobContext::new()),
            #[cfg(feature = "redis")]
            redis_persistence: None,
        }
    }

    /// Create a new job agent with custom context.
    ///
    /// The context provides jobs with access to services like email sender,
    /// database pool, and file storage.
    #[must_use]
    pub fn with_context(context: JobContext) -> Self {
        Self {
            queue: Arc::new(RwLock::new(JobQueue::new(10_000))),
            running: Arc::new(RwLock::new(HashMap::new())),
            dead_letter: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(JobHistory::new(1000))), // Keep last 1000 jobs
            metrics: Arc::new(RwLock::new(JobMetrics::default())),
            context: Arc::new(context),
            #[cfg(feature = "redis")]
            redis_persistence: None,
        }
    }

    /// Create a new job agent with Redis persistence.
    ///
    /// # Arguments
    ///
    /// * `context` - Job execution context with services
    /// * `redis_persistence` - Handle to Redis persistence agent
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use acton_reactive::prelude::*;
    /// use acton_htmx::jobs::{JobAgent, JobContext};
    /// use acton_htmx::jobs::agent::RedisPersistenceAgent;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut runtime = ActorRuntime::new().await?;
    ///
    /// // Spawn Redis persistence actor
    /// let redis_handle = RedisPersistenceAgent::spawn(
    ///     "redis://localhost:6379",
    ///     &mut runtime
    /// ).await?;
    ///
    /// // Create job agent with persistence
    /// let context = JobContext::new();
    /// let job_agent = JobAgent::with_persistence(context, redis_handle);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "redis")]
    #[must_use]
    pub fn with_persistence(context: JobContext, redis_persistence: ActorHandle) -> Self {
        Self {
            queue: Arc::new(RwLock::new(JobQueue::new(10_000))),
            running: Arc::new(RwLock::new(HashMap::new())),
            dead_letter: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(JobHistory::new(1000))), // Keep last 1000 jobs
            metrics: Arc::new(RwLock::new(JobMetrics::default())),
            context: Arc::new(context),
            redis_persistence: Some(redis_persistence),
        }
    }

    /// Get the job context.
    ///
    /// This provides access to services configured for job execution.
    #[must_use]
    pub const fn context(&self) -> &Arc<JobContext> {
        &self.context
    }

    /// Spawn job actor
    ///
    /// Uses in-memory queue. Redis persistence and retry logic will be added in Week 5.
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails
    pub async fn spawn(
        runtime: &mut ActorRuntime,
    ) -> anyhow::Result<ActorHandle> {
        let actor_config = ActorConfig::new(Ern::with_root("job_manager")?, None, None)?;
        let mut builder = runtime.new_actor_with_config::<Self>(actor_config);
        builder.model = Self::new();
        Self::configure_handlers(builder).await
    }

    /// Configure all message handlers for the job actor
    #[allow(clippy::too_many_lines)]
    async fn configure_handlers(mut builder: JobActorBuilder) -> anyhow::Result<ActorHandle> {
        builder
            // Enqueue a job (actor-to-actor with reply_envelope)
            .mutate_on::<EnqueueJob>(|actor, context| {
                let msg = context.message().clone();
                let reply_envelope = context.reply_envelope();

                debug!("Enqueueing job {} with priority {}", msg.id, msg.priority);

                let queued_job = QueuedJob {
                    id: msg.id,
                    job_type: msg.job_type,
                    payload: msg.payload,
                    priority: msg.priority,
                    max_retries: msg.max_retries,
                    timeout: msg.timeout,
                    enqueued_at: Utc::now(),
                    attempt: 0,
                };

                // Add to in-memory queue
                let result = actor.model.queue.write().enqueue(queued_job.clone());

                // Clone Redis persistence handle if available
                #[cfg(feature = "redis")]
                let redis_handle = actor.model.redis_persistence.clone();

                match result {
                    Ok(()) => {
                        actor.model.metrics.write().jobs_enqueued += 1;

                        // Send response via reply_envelope
                        let response = JobEnqueued { id: msg.id };
                        Reply::pending(async move {
                            // Persist to Redis if enabled (fire-and-forget)
                            #[cfg(feature = "redis")]
                            if let Some(redis) = redis_handle {
                                use persistence::PersistJob;
                                redis.send(PersistJob { job: queued_job }).await;
                            }

                            let _: () = reply_envelope.send(response).await;
                        })
                    }
                    Err(e) => {
                        warn!("Failed to enqueue job {}: {:?}", msg.id, e);
                        actor.model.metrics.write().jobs_rejected += 1;
                        Reply::ready()
                    }
                }
            })
            // Get job status (read-only with reply_envelope)
            .act_on::<GetJobStatus>(|actor, context| {
                let msg = context.message().clone();
                let reply_envelope = context.reply_envelope();

                // Clone data from actor before moving into async
                let status = actor.model.running.read().get(&msg.id).map_or_else(
                    || {
                        if actor.model.queue.read().contains(&msg.id) {
                            Some(JobStatus::Pending)
                        } else {
                            None
                        }
                    },
                    |status| Some(status.clone()),
                );

                Reply::pending(async move {
                    let response = JobStatusResponse {
                        id: msg.id,
                        status,
                    };
                    let _: () = reply_envelope.send(response).await;
                })
            })
            // Get metrics (read-only with reply_envelope - actor-to-actor pattern)
            .act_on::<GetMetrics>(|actor, context| {
                let reply_envelope = context.reply_envelope();
                let metrics = actor.model.metrics.read().clone();

                Reply::pending(async move {
                    let _: () = reply_envelope.send(metrics).await;
                })
            })
            // Get metrics (web handler pattern with oneshot channel)
            .act_on::<GetMetricsRequest>(|actor, context| {
                let response_tx = context.message().response_tx.clone();
                let metrics = actor.model.metrics.read().clone();

                Reply::pending(async move {
                    Self::send_metrics_response(response_tx, metrics).await;
                })
            })
            // Get job status (web handler pattern with oneshot channel)
            .act_on::<GetJobStatusRequest>(|actor, context| {
                let msg = context.message();
                let response_tx = msg.response_tx.clone();
                let job_id = msg.id;

                let status = actor.model.running.read().get(&job_id).cloned()
                    .or_else(|| {
                        if actor.model.queue.read().contains(&job_id) {
                            Some(JobStatus::Pending)
                        } else {
                            None
                        }
                    });

                Reply::pending(async move {
                    Self::send_status_response(response_tx, status).await;
                })
            })
            // Retry a failed job from dead letter queue
            .mutate_on::<RetryJobRequest>(|actor, context| {
                let msg = context.message();
                let response_tx = msg.response_tx.clone();
                let job_id = msg.id;

                // Try to move job from DLQ back to main queue
                let success = actor.model.dead_letter.write().remove(&job_id)
                    .and_then(|mut job| {
                        // Reset attempt counter for retry
                        job.attempt = 0;
                        actor.model.queue.write().enqueue(job).ok()
                    })
                    .is_some();

                Reply::pending(async move {
                    Self::send_bool_response(response_tx, success).await;
                })
            })
            // Retry all failed jobs from dead letter queue
            .mutate_on::<RetryAllFailedRequest>(|actor, context| {
                let response_tx = context.message().response_tx.clone();

                // Collect all jobs from DLQ
                let jobs: Vec<QueuedJob> = actor.model.dead_letter.write()
                    .drain()
                    .map(|(_, mut job)| {
                        // Reset attempt counter
                        job.attempt = 0;
                        job
                    })
                    .collect();

                // Re-enqueue all jobs
                let mut queue = actor.model.queue.write();
                let mut retried = 0;
                for job in jobs {
                    if queue.enqueue(job).is_ok() {
                        retried += 1;
                    }
                }

                Reply::pending(async move {
                    Self::send_usize_response(response_tx, retried).await;
                })
            })
            // Cancel a running or pending job
            .mutate_on::<CancelJobRequest>(|actor, context| {
                let msg = context.message();
                let response_tx = msg.response_tx.clone();
                let job_id = msg.id;

                // Try to remove from queue first
                let success = if actor.model.queue.write().remove(&job_id).is_some() {
                    true
                } else {
                    // If not in queue, check if it's running and mark for cancellation
                    actor.model.running.write().remove(&job_id).is_some()
                };

                Reply::pending(async move {
                    Self::send_bool_response(response_tx, success).await;
                })
            })
            // Clear the dead letter queue
            .mutate_on::<ClearDeadLetterQueueRequest>(|actor, context| {
                let response_tx = context.message().response_tx.clone();

                // Clear all jobs from DLQ
                let count = {
                    let mut dlq = actor.model.dead_letter.write();
                    let count = dlq.len();
                    dlq.clear();
                    count
                };

                // Update metrics
                actor.model.metrics.write().jobs_in_dlq = 0;

                Reply::pending(async move {
                    Self::send_usize_response(response_tx, count).await;
                })
            })
            // Get job history with pagination and search
            .act_on::<GetJobHistoryRequest>(|actor, context| {
                let msg = context.message();
                let response_tx = msg.response_tx.clone();
                let page = msg.page;
                let page_size = msg.page_size;
                let search_query = msg.search_query.clone();

                // Get paginated history from the actor's history store
                let (jobs, total_count) = actor
                    .model
                    .history
                    .read()
                    .get_page(page, page_size, search_query.as_deref());

                Reply::pending(async move {
                    let history_page = JobHistoryPage::new(jobs, page, page_size, total_count);
                    Self::send_history_response(response_tx, history_page).await;
                })
            });

        // Redis persistence is now handled by RedisPersistenceAgent (separate actor)
        // Messages are sent via fire-and-forget pattern when feature is enabled

        Ok(builder.start().await)
    }

    /// Send metrics response via oneshot channel.
    ///
    /// Helper method for web handler pattern responses.
    async fn send_metrics_response(
        response_tx: ResponseChannel<JobMetrics>,
        metrics: JobMetrics,
    ) {
        let mut guard = response_tx.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(metrics);
        }
    }

    /// Send job status response via oneshot channel.
    ///
    /// Helper method for web handler pattern responses.
    async fn send_status_response(
        response_tx: ResponseChannel<Option<JobStatus>>,
        status: Option<JobStatus>,
    ) {
        let mut guard = response_tx.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(status);
        }
    }

    /// Send boolean response via oneshot channel.
    ///
    /// Helper method for web handler pattern responses (retry, cancel operations).
    async fn send_bool_response(response_tx: ResponseChannel<bool>, success: bool) {
        let mut guard = response_tx.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(success);
        }
    }

    /// Send usize response via oneshot channel.
    ///
    /// Helper method for web handler pattern responses (count operations).
    async fn send_usize_response(response_tx: ResponseChannel<usize>, count: usize) {
        let mut guard = response_tx.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(count);
        }
    }

    /// Send job history page response via oneshot channel.
    ///
    /// Helper method for web handler pattern responses (history operations).
    async fn send_history_response(
        response_tx: ResponseChannel<JobHistoryPage>,
        history: JobHistoryPage,
    ) {
        let mut guard = response_tx.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(history);
        }
    }
}


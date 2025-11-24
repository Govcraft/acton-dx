//! Job processing agent using acton-reactive.

pub(crate) mod messages;
pub(crate) mod persistence;
pub(crate) mod queue;
#[cfg(feature = "redis")]
pub mod redis_agent;
pub mod scheduled;

pub use messages::{
    EnqueueJob, GetJobStatusRequest, GetMetricsRequest, JobEnqueued, JobMetrics, ResponseChannel,
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

use messages::{GetJobStatus, GetMetrics, JobStatusResponse};
use queue::{JobQueue, QueuedJob};

// Type alias for the ManagedAgent builder type
type JobAgentBuilder = ManagedAgent<Idle, JobAgent>;

/// Background job processing agent.
///
/// Manages a queue of background jobs with:
/// - Priority-based execution
/// - Redis persistence (via dedicated `RedisPersistenceAgent`)
/// - Automatic retry with exponential backoff
/// - Dead letter queue for failed jobs
/// - Graceful shutdown
/// - Service access via [`JobContext`](crate::jobs::JobContext)
#[derive(Clone)]
pub struct JobAgent {
    /// In-memory priority queue.
    queue: Arc<RwLock<JobQueue>>,
    /// Currently running jobs.
    running: Arc<RwLock<HashMap<JobId, JobStatus>>>,
    /// Job metrics.
    metrics: Arc<RwLock<JobMetrics>>,
    /// Job execution context with services.
    ///
    /// Provides jobs with access to email sender, database pool, file storage, etc.
    context: Arc<JobContext>,
    /// Handle to Redis persistence agent (optional, for persistence).
    #[cfg(feature = "redis")]
    redis_persistence: Option<AgentHandle>,
}

impl std::fmt::Debug for JobAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("JobAgent");
        debug_struct
            .field("queue", &"<JobQueue>")
            .field("running", &self.running.read().len())
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
    /// let mut runtime = AgentRuntime::new().await?;
    ///
    /// // Spawn Redis persistence agent
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
    pub fn with_persistence(context: JobContext, redis_persistence: AgentHandle) -> Self {
        Self {
            queue: Arc::new(RwLock::new(JobQueue::new(10_000))),
            running: Arc::new(RwLock::new(HashMap::new())),
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

    /// Spawn job agent
    ///
    /// Uses in-memory queue. Redis persistence and retry logic will be added in Week 5.
    ///
    /// # Errors
    ///
    /// Returns error if agent initialization fails
    pub async fn spawn(
        runtime: &mut AgentRuntime,
    ) -> anyhow::Result<AgentHandle> {
        let agent_config = AgentConfig::new(Ern::with_root("job_manager")?, None, None)?;
        let mut builder = runtime.new_agent_with_config::<Self>(agent_config).await;
        builder.model = Self::new();
        Self::configure_handlers(builder).await
    }

    /// Configure all message handlers for the job agent
    #[allow(clippy::too_many_lines)]
    async fn configure_handlers(mut builder: JobAgentBuilder) -> anyhow::Result<AgentHandle> {
        builder
            // Enqueue a job (agent-to-agent with reply_envelope)
            .mutate_on::<EnqueueJob>(|agent, envelope| {
                let msg = envelope.message().clone();
                let reply_envelope = envelope.reply_envelope();

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
                let result = agent.model.queue.write().enqueue(queued_job.clone());

                // Clone Redis persistence handle if available
                #[cfg(feature = "redis")]
                let redis_handle = agent.model.redis_persistence.clone();

                match result {
                    Ok(()) => {
                        agent.model.metrics.write().jobs_enqueued += 1;

                        // Send response via reply_envelope
                        let response = JobEnqueued { id: msg.id };
                        AgentReply::from_async(async move {
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
                        agent.model.metrics.write().jobs_rejected += 1;
                        AgentReply::immediate()
                    }
                }
            })
            // Get job status (read-only with reply_envelope)
            .act_on::<GetJobStatus>(|agent, envelope| {
                let msg = envelope.message().clone();
                let reply_envelope = envelope.reply_envelope();

                // Clone data from agent before moving into async
                let status = agent.model.running.read().get(&msg.id).map_or_else(
                    || {
                        if agent.model.queue.read().contains(&msg.id) {
                            Some(JobStatus::Pending)
                        } else {
                            None
                        }
                    },
                    |status| Some(status.clone()),
                );

                Box::pin(async move {
                    let response = JobStatusResponse {
                        id: msg.id,
                        status,
                    };
                    let _: () = reply_envelope.send(response).await;
                })
            })
            // Get metrics (read-only with reply_envelope - agent-to-agent pattern)
            .act_on::<GetMetrics>(|agent, envelope| {
                let reply_envelope = envelope.reply_envelope();
                let metrics = agent.model.metrics.read().clone();

                Box::pin(async move {
                    let _: () = reply_envelope.send(metrics).await;
                })
            })
            // Get metrics (web handler pattern with oneshot channel)
            .act_on::<GetMetricsRequest>(|agent, envelope| {
                let response_tx = envelope.message().response_tx.clone();
                let metrics = agent.model.metrics.read().clone();

                Box::pin(async move {
                    Self::send_metrics_response(response_tx, metrics).await;
                })
            })
            // Get job status (web handler pattern with oneshot channel)
            .act_on::<GetJobStatusRequest>(|agent, envelope| {
                let msg = envelope.message();
                let response_tx = msg.response_tx.clone();
                let job_id = msg.id;

                let status = agent.model.running.read().get(&job_id).cloned()
                    .or_else(|| {
                        if agent.model.queue.read().contains(&job_id) {
                            Some(JobStatus::Pending)
                        } else {
                            None
                        }
                    });

                Box::pin(async move {
                    Self::send_status_response(response_tx, status).await;
                })
            });

        // Redis persistence is now handled by RedisPersistenceAgent (separate agent)
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
}


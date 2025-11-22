//! Job processing agent using acton-reactive.

mod messages;
mod queue;

pub use messages::{EnqueueJob, JobEnqueued, JobMetrics};

use super::{JobId, JobStatus};
use acton_reactive::prelude::*;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use messages::*;
use queue::{JobQueue, QueuedJob};

// Type alias for the ManagedAgent builder type
type JobAgentBuilder = ManagedAgent<Idle, JobAgent>;

/// Background job processing agent.
///
/// Manages a queue of background jobs with:
/// - Priority-based execution
/// - Redis persistence (Week 5)
/// - Automatic retry with exponential backoff (Week 5)
/// - Dead letter queue for failed jobs (Week 5)
/// - Graceful shutdown
#[derive(Debug, Clone)]
pub struct JobAgent {
    /// In-memory priority queue.
    pub(crate) queue: Arc<RwLock<JobQueue>>,
    /// Currently running jobs.
    pub(crate) running: Arc<RwLock<HashMap<JobId, JobStatus>>>,
    /// Job metrics.
    pub(crate) metrics: Arc<RwLock<JobMetrics>>,
}

impl Default for JobAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl JobAgent {
    /// Create a new job agent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(RwLock::new(JobQueue::new(10_000))),
            running: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(JobMetrics::default())),
        }
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

                match result {
                    Ok(()) => {
                        agent.model.metrics.write().jobs_enqueued += 1;

                        // Send response via reply_envelope
                        let response = JobEnqueued { id: msg.id };
                        AgentReply::from_async(async move {
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
                let status = if let Some(status) = agent.model.running.read().get(&msg.id) {
                    Some(status.clone())
                } else if agent.model.queue.read().contains(&msg.id) {
                    Some(JobStatus::Pending)
                } else {
                    None
                };

                Box::pin(async move {
                    let response = JobStatusResponse {
                        id: msg.id,
                        status,
                    };
                    let _: () = reply_envelope.send(response).await;
                })
            })
            // Get metrics (read-only with reply_envelope)
            .act_on::<GetMetrics>(|agent, envelope| {
                let reply_envelope = envelope.reply_envelope();
                let metrics = agent.model.metrics.read().clone();

                Box::pin(async move {
                    let _: () = reply_envelope.send(metrics).await;
                })
            });

        Ok(builder.start().await)
    }
}


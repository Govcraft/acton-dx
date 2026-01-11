//! Scheduled job management agent.

use super::messages::EnqueueJob;
use crate::htmx::jobs::{JobError, JobId, JobSchedule};
use acton_reactive::prelude::*;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

/// A scheduled job entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJobEntry {
    /// Unique scheduled job identifier.
    pub id: JobId,
    /// Job type name.
    pub job_type: String,
    /// Serialized job payload.
    pub payload: Vec<u8>,
    /// Job schedule.
    pub schedule: JobSchedule,
    /// Job priority.
    pub priority: i32,
    /// Maximum retries for enqueued jobs.
    pub max_retries: u32,
    /// Timeout for enqueued jobs.
    pub timeout: Duration,
    /// Next scheduled execution time.
    pub next_execution: DateTime<Utc>,
    /// Number of times this job has been executed.
    pub execution_count: u64,
    /// Whether this scheduled job is enabled.
    pub enabled: bool,
}

/// Messages for the scheduled job agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduledJobMessage {
    /// Register a new scheduled job.
    RegisterScheduledJob {
        /// Job type name.
        job_type: String,
        /// Serialized job payload.
        payload: Vec<u8>,
        /// Job schedule.
        schedule: JobSchedule,
        /// Job priority.
        priority: i32,
        /// Maximum retries.
        max_retries: u32,
        /// Job timeout.
        timeout: Duration,
    },

    /// Unregister a scheduled job.
    UnregisterScheduledJob {
        /// Scheduled job ID to remove.
        id: JobId,
    },

    /// Enable/disable a scheduled job.
    SetScheduledJobEnabled {
        /// Scheduled job ID.
        id: JobId,
        /// Whether to enable or disable.
        enabled: bool,
    },

    /// Trigger scheduled job processing (internal, sent by timer).
    ProcessScheduledJobs,

    /// Get all scheduled jobs.
    GetScheduledJobs,
}

/// Response messages from scheduled job agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduledJobResponse {
    /// Scheduled job was registered successfully.
    JobRegistered {
        /// The ID of the registered job.
        id: JobId,
    },

    /// Scheduled job was unregistered.
    JobUnregistered,

    /// Scheduled job enabled status was updated.
    EnabledUpdated,

    /// List of scheduled jobs.
    ScheduledJobs(Vec<ScheduledJobEntry>),
}

/// Scheduled job management actor.
///
/// Manages recurring, delayed, and cron-based job scheduling by:
/// - Storing scheduled job definitions
/// - Calculating next execution times
/// - Enqueueing jobs at the scheduled time
/// - Tracking execution counts
#[derive(Debug, Clone)]
pub struct ScheduledJobAgent {
    /// All scheduled jobs indexed by ID.
    scheduled_jobs: Arc<RwLock<HashMap<JobId, ScheduledJobEntry>>>,
    /// Handle to the job queue actor.
    job_agent_handle: Option<ActorHandle>,
}

impl Default for ScheduledJobAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduledJobAgent {
    /// Create a new scheduled job agent.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scheduled_jobs: Arc::new(RwLock::new(HashMap::new())),
            job_agent_handle: None,
        }
    }

    /// Spawn the scheduled job actor.
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails.
    pub async fn spawn(
        runtime: &mut ActorRuntime,
        job_agent_handle: ActorHandle,
    ) -> anyhow::Result<ActorHandle> {
        let actor_config =
            ActorConfig::new(Ern::with_root("scheduled_job_manager")?, None, None)?;
        let mut builder = runtime
            .new_actor_with_config::<Self>(actor_config);

        // Set the job actor handle
        builder.model.job_agent_handle = Some(job_agent_handle);

        // Configure message handlers
        builder.mutate_on::<ScheduledJobMessage>(|actor, context| {
            let msg = context.message().clone();
            let reply_envelope = context.reply_envelope();

            match msg {
                ScheduledJobMessage::RegisterScheduledJob {
                    job_type,
                    payload,
                    schedule,
                    priority,
                    max_retries,
                    timeout,
                } => {
                    let id = JobId::new();
                    let next_execution = schedule
                        .next_execution(Utc::now())
                        .unwrap_or_else(Utc::now);

                    let entry = ScheduledJobEntry {
                        id,
                        job_type,
                        payload,
                        schedule,
                        priority,
                        max_retries,
                        timeout,
                        next_execution,
                        execution_count: 0,
                        enabled: true,
                    };

                    actor.model.scheduled_jobs.write().insert(id, entry);
                    info!("Registered scheduled job: {}", id);

                    Reply::pending(async move {
                        let response = ScheduledJobResponse::JobRegistered { id };
                        let _: () = reply_envelope.send(response).await;
                    })
                }
                ScheduledJobMessage::UnregisterScheduledJob { id } => {
                    actor.model.scheduled_jobs.write().remove(&id);
                    info!("Unregistered scheduled job: {}", id);

                    Reply::pending(async move {
                        let response = ScheduledJobResponse::JobUnregistered;
                        let _: () = reply_envelope.send(response).await;
                    })
                }
                ScheduledJobMessage::SetScheduledJobEnabled { id, enabled } => {
                    if let Some(entry) = actor.model.scheduled_jobs.write().get_mut(&id) {
                        entry.enabled = enabled;
                        info!("Set scheduled job {} enabled={}", id, enabled);
                    }

                    Reply::pending(async move {
                        let response = ScheduledJobResponse::EnabledUpdated;
                        let _: () = reply_envelope.send(response).await;
                    })
                }
                ScheduledJobMessage::ProcessScheduledJobs => {
                    // Clone what we need for processing
                    let scheduled_jobs = actor.model.scheduled_jobs.clone();
                    let job_handle = actor.model.job_agent_handle.clone();

                    // Process in async block
                    Reply::pending(async move {
                        Self::process_scheduled_jobs_async(scheduled_jobs, job_handle).await;
                    })
                }
                ScheduledJobMessage::GetScheduledJobs => {
                    let jobs = actor
                        .model
                        .scheduled_jobs
                        .read()
                        .values()
                        .cloned()
                        .collect();

                    Reply::pending(async move {
                        let response = ScheduledJobResponse::ScheduledJobs(jobs);
                        let _: () = reply_envelope.send(response).await;
                    })
                }
            }
        });

        Ok(builder.start().await)
    }

    /// Process all scheduled jobs and enqueue those that are ready (async).
    #[allow(clippy::cognitive_complexity)]
    async fn process_scheduled_jobs_async(
        scheduled_jobs: Arc<RwLock<HashMap<JobId, ScheduledJobEntry>>>,
        job_handle: Option<ActorHandle>,
    ) {
        let now = Utc::now();
        let mut jobs_to_enqueue = Vec::new();

        // Find jobs that need to be executed
        {
            let mut jobs = scheduled_jobs.write();
            for entry in jobs.values_mut() {
                if !entry.enabled {
                    continue;
                }

                if entry.next_execution <= now {
                    // Check if schedule allows more executions
                    if !entry.schedule.has_more_executions(entry.execution_count) {
                        debug!("Scheduled job {} has no more executions", entry.id);
                        entry.enabled = false;
                        continue;
                    }

                    jobs_to_enqueue.push(entry.clone());

                    // Update execution count and next execution time
                    entry.execution_count += 1;
                    if let Some(next) = entry.schedule.next_execution(now) {
                        entry.next_execution = next;
                    } else {
                        // No more executions
                        entry.enabled = false;
                    }
                }
            }
        }

        // Enqueue jobs
        if let Some(job_agent) = job_handle {
            for entry in jobs_to_enqueue {
                debug!("Enqueueing scheduled job: {}", entry.id);

                let enqueue_msg = EnqueueJob {
                    id: JobId::new(), // New ID for each execution
                    job_type: entry.job_type.clone(),
                    payload: entry.payload.clone(),
                    priority: entry.priority,
                    max_retries: entry.max_retries,
                    timeout: entry.timeout,
                };

                // Send message to job agent using handle
                job_agent.send(enqueue_msg).await;
                debug!("Successfully enqueued scheduled job");
            }
        } else {
            error!("Job agent handle not set - cannot enqueue scheduled jobs");
        }
    }
}

/// Start a background task that triggers scheduled job processing every minute.
///
/// # Errors
///
/// Returns error if the scheduled job agent handle is invalid.
pub async fn start_scheduler_loop(scheduler_handle: ActorHandle) -> Result<(), JobError> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));

        loop {
            interval.tick().await;

            debug!("Triggering scheduled job processing");
            let msg = ScheduledJobMessage::ProcessScheduledJobs;

            scheduler_handle.send(msg).await;
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_job_entry_creation() {
        let entry = ScheduledJobEntry {
            id: JobId::new(),
            job_type: "TestJob".to_string(),
            payload: vec![1, 2, 3],
            schedule: JobSchedule::after(Duration::from_secs(60)),
            priority: 0,
            max_retries: 3,
            timeout: Duration::from_secs(300),
            next_execution: Utc::now(),
            execution_count: 0,
            enabled: true,
        };

        assert_eq!(entry.job_type, "TestJob");
        assert!(entry.enabled);
        assert_eq!(entry.execution_count, 0);
    }

    #[test]
    fn test_scheduled_job_serialization() {
        let entry = ScheduledJobEntry {
            id: JobId::new(),
            job_type: "TestJob".to_string(),
            payload: vec![1, 2, 3],
            schedule: JobSchedule::every(Duration::from_secs(60)),
            priority: 0,
            max_retries: 3,
            timeout: Duration::from_secs(300),
            next_execution: Utc::now(),
            execution_count: 5,
            enabled: true,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ScheduledJobEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.job_type, deserialized.job_type);
        assert_eq!(entry.execution_count, deserialized.execution_count);
    }
}

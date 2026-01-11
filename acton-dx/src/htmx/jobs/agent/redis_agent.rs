//! Redis persistence agent for background jobs.
//!
//! This agent handles all Redis IO operations for job persistence,
//! using the acton-reactive pattern to properly handle Send + Sync bounds.
//!
//! All handlers use `act_on` for concurrent execution since Redis operations
//! only modify external state (the Redis database), not agent state.

use super::persistence::{
    MarkJobCompleted, MarkJobFailed, MoveToDeadLetterQueue, PersistJob,
};
use super::queue::QueuedJob;
use crate::htmx::jobs::{JobId, JobStatus};
use acton_reactive::prelude::*;
use redis::AsyncCommands;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Redis persistence agent that handles all job persistence operations.
///
/// This agent encapsulates Redis IO operations to handle Send + Sync bounds properly.
/// The Redis connection is cloneable (Arc-based internally), allowing concurrent operations.
///
/// # Handler Strategy
///
/// All handlers use `act_on` (not `mutate_on`) because:
/// - Redis operations modify external state, not agent state
/// - `act_on` allows concurrent execution for better throughput
/// - Agent state only needs immutable access to the connection
///
/// # Communication Pattern
///
/// JobAgent communicates via fire-and-forget messages:
/// - `PersistJob` - Persist job to Redis
/// - `MarkJobCompleted` - Mark job as completed
/// - `MarkJobFailed` - Mark job as failed
/// - `MoveToDeadLetterQueue` - Move job to DLQ
#[derive(Clone, Default)]
pub struct RedisPersistenceAgent {
    /// Redis connection (cloneable via Arc internally).
    ///
    /// None in Default impl - always set via spawn().
    redis_conn: Option<redis::aio::MultiplexedConnection>,
    /// Count of operations performed (for metrics).
    operations_count: Arc<AtomicUsize>,
}

impl std::fmt::Debug for RedisPersistenceAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisPersistenceAgent")
            .field("redis_conn", &self.redis_conn.is_some())
            .field("operations_count", &self.operations_count.load(Ordering::Relaxed))
            .finish()
    }
}

impl RedisPersistenceAgent {
    /// Create and spawn a new Redis persistence agent.
    ///
    /// # Arguments
    ///
    /// * `redis_url` - Redis connection URL (e.g., "redis://localhost:6379")
    /// * `runtime` - Agent runtime for spawning the agent
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Redis connection fails
    /// - Agent spawning fails
    ///
    /// # Panics
    ///
    /// Panics if handler configuration fails, which should not occur in normal operation.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use acton_reactive::prelude::*;
    /// use acton_htmx::jobs::agent::redis_agent::RedisPersistenceAgent;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut runtime = ActorRuntime::new().await?;
    /// let handle = RedisPersistenceAgent::spawn(
    ///     "redis://localhost:6379",
    ///     &mut runtime
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn spawn(
        redis_url: &str,
        runtime: &mut ActorRuntime,
    ) -> anyhow::Result<ActorHandle> {
        // Create Redis connection
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_multiplexed_async_connection().await?;

        debug!("Connected to Redis at {}", redis_url);

        // Spawn actor using closure pattern
        runtime
            .spawn_actor(|mut actor: ManagedActor<Idle, Self>| {
                // Set model with Redis connection
                actor.model = Self {
                    redis_conn: Some(conn),
                    operations_count: Arc::new(AtomicUsize::new(0)),
                };

                Box::pin(async move {
                    Self::configure_handlers(actor).await.expect("Failed to configure handlers")
                })
            })
            .await
    }

    /// Configure all message handlers for the persistence actor.
    ///
    /// All handlers use `act_on` for concurrent execution since they only
    /// perform external IO without modifying actor state.
    async fn configure_handlers(
        mut builder: ManagedActor<Idle, Self>,
    ) -> anyhow::Result<ActorHandle> {
        builder
            // Persist job to Redis (fire-and-forget)
            .act_on::<PersistJob>(|actor, context| {
                let conn_opt = actor.model.redis_conn.clone();
                let job = context.message().job.clone();
                let ops_count = actor.model.operations_count.clone();

                // Spawn as tokio task to satisfy Sync bound
                Reply::pending(async move {
                    tokio::spawn(async move {
                        if let Some(mut conn) = conn_opt {
                            match persist_job_impl(&mut conn, &job).await {
                                Ok(()) => {
                                    ops_count.fetch_add(1, Ordering::Relaxed);
                                    debug!("Successfully persisted job {}", job.id);
                                }
                                Err(e) => {
                                    error!("Failed to persist job {}: {:?}", job.id, e);
                                }
                            }
                        }
                    });
                })
            })

            // Mark job as completed (fire-and-forget)
            .act_on::<MarkJobCompleted>(|actor, context| {
                let conn_opt = actor.model.redis_conn.clone();
                let msg = context.message().clone();
                let ops_count = actor.model.operations_count.clone();

                // Spawn as tokio task to satisfy Sync bound
                Reply::pending(async move {
                    tokio::spawn(async move {
                        if let Some(mut conn) = conn_opt {
                            match mark_completed_impl(&mut conn, msg.id, msg.execution_time_ms).await {
                                Ok(()) => {
                                    ops_count.fetch_add(1, Ordering::Relaxed);
                                    debug!("Successfully marked job {} as completed", msg.id);
                                }
                                Err(e) => {
                                    error!("Failed to mark job {} as completed: {:?}", msg.id, e);
                                }
                            }
                        }
                    });
                })
            })

            // Mark job as failed (fire-and-forget)
            .act_on::<MarkJobFailed>(|actor, context| {
                let conn_opt = actor.model.redis_conn.clone();
                let msg = context.message().clone();
                let ops_count = actor.model.operations_count.clone();

                // Spawn as tokio task to satisfy Sync bound
                Reply::pending(async move {
                    tokio::spawn(async move {
                        if let Some(mut conn) = conn_opt {
                            match mark_failed_impl(&mut conn, msg.id, &msg.error, msg.attempt).await {
                                Ok(()) => {
                                    ops_count.fetch_add(1, Ordering::Relaxed);
                                    debug!("Successfully marked job {} as failed", msg.id);
                                }
                                Err(e) => {
                                    error!("Failed to mark job {} as failed: {:?}", msg.id, e);
                                }
                            }
                        }
                    });
                })
            })

            // Move job to dead letter queue (fire-and-forget)
            .act_on::<MoveToDeadLetterQueue>(|actor, context| {
                let conn_opt = actor.model.redis_conn.clone();
                let msg = context.message().clone();
                let ops_count = actor.model.operations_count.clone();

                // Spawn as tokio task to satisfy Sync bound
                Reply::pending(async move {
                    tokio::spawn(async move {
                        if let Some(mut conn) = conn_opt {
                            match move_to_dlq_impl(&mut conn, msg.id, &msg.job, &msg.error).await {
                                Ok(()) => {
                                    ops_count.fetch_add(1, Ordering::Relaxed);
                                    warn!("Moved job {} to DLQ: {}", msg.id, msg.error);
                                }
                                Err(e) => {
                                    error!("Failed to move job {} to DLQ: {:?}", msg.id, e);
                                }
                            }
                        }
                    });
                })
            });

        Ok(builder.start().await)
    }
}

/// Implementation function for persisting a job to Redis.
async fn persist_job_impl(
    redis: &mut redis::aio::MultiplexedConnection,
    job: &QueuedJob,
) -> Result<(), redis::RedisError> {
    let key = format!("job:{}", job.id);
    let json = serde_json::to_string(job).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "serialization error",
            e.to_string(),
        ))
    })?;

    // Store job data with 7 day expiry
    let _: () = redis.set_ex(&key, json, 604_800).await?;

    // Add to pending queue
    let _: usize = redis.lpush("queue:pending", job.id.to_string()).await?;

    Ok(())
}

/// Implementation function for marking job as completed in Redis.
async fn mark_completed_impl(
    redis: &mut redis::aio::MultiplexedConnection,
    id: JobId,
    execution_time_ms: u64,
) -> Result<(), redis::RedisError> {
    let key = format!("job:{id}");

    // Update status
    let status = JobStatus::Completed {
        completed_at: chrono::Utc::now(),
    };
    let status_json = serde_json::to_string(&status).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "serialization error",
            e.to_string(),
        ))
    })?;

    let _: () = redis.hset(&key, "status", status_json).await?;
    let _: () = redis.hset(&key, "execution_time_ms", execution_time_ms).await?;

    // Remove from pending, add to completed
    let _: usize = redis.lrem("queue:pending", 1, id.to_string()).await?;
    let _: usize = redis.lpush("queue:completed", id.to_string()).await?;

    Ok(())
}

/// Implementation function for marking job as failed in Redis.
async fn mark_failed_impl(
    redis: &mut redis::aio::MultiplexedConnection,
    id: JobId,
    error: &str,
    attempt: u32,
) -> Result<(), redis::RedisError> {
    let key = format!("job:{id}");

    // Update status
    let status = JobStatus::Failed {
        failed_at: chrono::Utc::now(),
        attempts: attempt,
        error: error.to_string(),
    };
    let status_json = serde_json::to_string(&status).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "serialization error",
            e.to_string(),
        ))
    })?;

    let _: () = redis.hset(&key, "status", status_json).await?;
    let _: () = redis.hset(&key, "attempts", attempt).await?;

    Ok(())
}

/// Implementation function for moving job to dead letter queue.
async fn move_to_dlq_impl(
    redis: &mut redis::aio::MultiplexedConnection,
    id: JobId,
    job: &QueuedJob,
    error: &str,
) -> Result<(), redis::RedisError> {
    let dlq_key = format!("dlq:{id}");
    let json = serde_json::to_string(job).map_err(|e| {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "serialization error",
            e.to_string(),
        ))
    })?;

    // Store in DLQ with permanent retention
    let _: () = redis.hset(&dlq_key, "job", json).await?;
    let _: () = redis.hset(&dlq_key, "error", error).await?;
    let _: () = redis.hset(&dlq_key, "moved_at", chrono::Utc::now().to_rfc3339()).await?;

    // Add to DLQ list
    let _: usize = redis.lpush("queue:dlq", id.to_string()).await?;

    // Remove from pending
    let _: usize = redis.lrem("queue:pending", 1, id.to_string()).await?;

    Ok(())
}

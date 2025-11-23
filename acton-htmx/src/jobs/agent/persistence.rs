//! Redis persistence for jobs using act_on handlers.

use super::queue::QueuedJob;
use crate::jobs::{JobId, JobStatus};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

#[cfg(feature = "redis")]
use redis::AsyncCommands;

/// Message to persist a job to Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistJob {
    /// The job to persist.
    pub job: QueuedJob,
}

/// Message to mark a job as completed in Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkJobCompleted {
    /// Job ID.
    pub id: JobId,
    /// Execution time in milliseconds.
    pub execution_time_ms: u64,
}

/// Message to mark a job as failed in Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkJobFailed {
    /// Job ID.
    pub id: JobId,
    /// Error message.
    pub error: String,
    /// Current attempt number.
    pub attempt: u32,
}

/// Message to move a job to the dead letter queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveToDeadLetterQueue {
    /// Job ID.
    pub id: JobId,
    /// Job data.
    pub job: QueuedJob,
    /// Final error message.
    pub error: String,
}

#[cfg(feature = "redis")]
/// Persist a job to Redis (called from act_on handler).
#[allow(dead_code)] // Will be used when Redis handlers are enabled
pub(super) async fn persist_job_to_redis(
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

    debug!("Persisted job {} to Redis", job.id);
    Ok(())
}

#[cfg(feature = "redis")]
/// Mark job as completed in Redis.
#[allow(dead_code)] // Will be used when Redis handlers are enabled
pub(super) async fn mark_completed_in_redis(
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

    debug!("Marked job {} as completed in Redis", id);
    Ok(())
}

#[cfg(feature = "redis")]
/// Mark job as failed in Redis.
#[allow(dead_code)] // Will be used when Redis handlers are enabled
pub(super) async fn mark_failed_in_redis(
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

    debug!("Marked job {} as failed in Redis", id);
    Ok(())
}

#[cfg(feature = "redis")]
/// Move job to dead letter queue.
#[allow(dead_code)] // Will be used when Redis handlers are enabled
pub(super) async fn move_to_dlq_in_redis(
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

    error!("Moved job {} to dead letter queue: {}", id, error);
    Ok(())
}

#[cfg(not(feature = "redis"))]
/// Stub implementation when Redis feature is disabled.
#[allow(dead_code)]
pub(super) async fn persist_job_to_redis(_job: &QueuedJob) -> Result<(), String> {
    Ok(())
}

#[cfg(not(feature = "redis"))]
/// Stub implementation when Redis feature is disabled.
#[allow(dead_code)]
pub(super) async fn mark_completed_in_redis(_id: JobId, _execution_time_ms: u64) -> Result<(), String> {
    Ok(())
}

#[cfg(not(feature = "redis"))]
/// Stub implementation when Redis feature is disabled.
#[allow(dead_code)]
pub(super) async fn mark_failed_in_redis(_id: JobId, _error: &str, _attempt: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(not(feature = "redis"))]
/// Stub implementation when Redis feature is disabled.
#[allow(dead_code)]
pub(super) async fn move_to_dlq_in_redis(_id: JobId, _job: &QueuedJob, _error: &str) -> Result<(), String> {
    Ok(())
}

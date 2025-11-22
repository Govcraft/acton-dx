//! Job-related error types.

use thiserror::Error;

/// Result type for job operations.
pub type JobResult<T> = Result<T, JobError>;

/// Errors that can occur during job processing.
#[derive(Debug, Error)]
pub enum JobError {
    /// Job execution failed.
    #[error("job execution failed: {0}")]
    ExecutionFailed(String),

    /// Job timed out.
    #[error("job timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Job exceeded maximum retry attempts.
    #[error("job failed after {0} retries")]
    MaxRetriesExceeded(u32),

    /// Serialization error.
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Redis error.
    #[cfg(feature = "redis")]
    #[error("redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    /// Job not found.
    #[error("job not found: {0}")]
    NotFound(String),

    /// Job queue is full.
    #[error("job queue is full (max: {0})")]
    QueueFull(usize),

    /// Job agent not available.
    #[error("job agent not available")]
    AgentUnavailable,

    /// Other error.
    #[error("{0}")]
    Other(String),
}

impl From<String> for JobError {
    fn from(s: String) -> Self {
        Self::ExecutionFailed(s)
    }
}

impl From<&str> for JobError {
    fn from(s: &str) -> Self {
        Self::ExecutionFailed(s.to_string())
    }
}

impl From<anyhow::Error> for JobError {
    fn from(err: anyhow::Error) -> Self {
        Self::ExecutionFailed(err.to_string())
    }
}

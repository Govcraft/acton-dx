//! Core job trait and types.

use super::JobResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use uuid::Uuid;

/// Unique identifier for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(Uuid);

impl JobId {
    /// Create a new random job ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the underlying UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for JobId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<JobId> for Uuid {
    fn from(id: JobId) -> Self {
        id.0
    }
}

/// A background job that can be executed asynchronously.
///
/// # Type Parameters
///
/// Jobs are generic over their result type, which must be `Send + Sync`.
///
/// # Example
///
/// ```rust
/// use acton_htmx::jobs::{Job, JobContext, JobResult};
/// use async_trait::async_trait;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// pub struct SendEmailJob {
///     to: String,
///     subject: String,
///     body: String,
/// }
///
/// #[async_trait]
/// impl Job for SendEmailJob {
///     type Result = ();
///
///     async fn execute(&self, ctx: &JobContext) -> JobResult<Self::Result> {
///         // Send email using ctx.email_client
///         println!("Sending email to: {}", self.to);
///         Ok(())
///     }
///
///     fn max_retries(&self) -> u32 {
///         5 // Retry up to 5 times for transient failures
///     }
///
///     fn timeout(&self) -> Duration {
///         Duration::from_secs(30) // 30 second timeout
///     }
/// }
/// ```
#[async_trait]
pub trait Job: Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static {
    /// The result type returned by this job.
    type Result: Send + Sync;

    /// Execute the job.
    ///
    /// This will be fully implemented in Week 5 when job execution is added.
    ///
    /// # Errors
    ///
    /// Returns an error if the job execution fails. The job will be retried
    /// according to `max_retries()` with exponential backoff.
    async fn execute(&self) -> JobResult<Self::Result>;

    /// Maximum number of retry attempts.
    ///
    /// Default: 3 retries
    fn max_retries(&self) -> u32 {
        3
    }

    /// Timeout for job execution.
    ///
    /// If the job takes longer than this duration, it will be cancelled
    /// and marked as failed.
    ///
    /// Default: 5 minutes
    fn timeout(&self) -> Duration {
        Duration::from_secs(300)
    }

    /// Priority for job execution (higher = more important).
    ///
    /// Jobs with higher priority will be executed before lower priority jobs
    /// when multiple jobs are queued.
    ///
    /// Default: 0 (normal priority)
    fn priority(&self) -> i32 {
        0
    }

    /// Job type name for logging and debugging.
    ///
    /// Default: Returns the type name
    fn job_type(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id_creation() {
        let id1 = JobId::new();
        let id2 = JobId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_job_id_display() {
        let id = JobId::new();
        let display = format!("{id}");
        assert!(!display.is_empty());
        assert!(Uuid::parse_str(&display).is_ok());
    }

    #[test]
    fn test_job_id_uuid_conversion() {
        let uuid = Uuid::new_v4();
        let job_id = JobId::from(uuid);
        let converted: Uuid = job_id.into();
        assert_eq!(uuid, converted);
    }
}

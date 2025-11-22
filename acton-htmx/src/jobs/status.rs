//! Job status tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a background job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job is queued and waiting to be executed.
    Pending,

    /// Job is currently being executed.
    Running {
        /// When the job started executing.
        started_at: DateTime<Utc>,
    },

    /// Job completed successfully.
    Completed {
        /// When the job completed.
        completed_at: DateTime<Utc>,
    },

    /// Job failed and is being retried.
    Retrying {
        /// Number of attempts so far.
        attempt: u32,
        /// When the job last failed.
        failed_at: DateTime<Utc>,
        /// When the next retry will occur.
        retry_at: DateTime<Utc>,
        /// Error message from the last failure.
        error: String,
    },

    /// Job failed permanently after exhausting retries.
    Failed {
        /// When the job finally failed.
        failed_at: DateTime<Utc>,
        /// Number of attempts made.
        attempts: u32,
        /// Final error message.
        error: String,
    },

    /// Job was cancelled.
    Cancelled {
        /// When the job was cancelled.
        cancelled_at: DateTime<Utc>,
    },
}

impl JobStatus {
    /// Check if the job is in a terminal state (completed, failed, or cancelled).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }

    /// Check if the job is currently running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// Check if the job is pending.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Check if the job is retrying.
    #[must_use]
    pub const fn is_retrying(&self) -> bool {
        matches!(self, Self::Retrying { .. })
    }

    /// Get a human-readable status name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running { .. } => "running",
            Self::Completed { .. } => "completed",
            Self::Retrying { .. } => "retrying",
            Self::Failed { .. } => "failed",
            Self::Cancelled { .. } => "cancelled",
        }
    }
}

impl Default for JobStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_is_terminal() {
        assert!(!JobStatus::Pending.is_terminal());
        assert!(!JobStatus::Running {
            started_at: Utc::now()
        }
        .is_terminal());
        assert!(JobStatus::Completed {
            completed_at: Utc::now()
        }
        .is_terminal());
        assert!(JobStatus::Failed {
            failed_at: Utc::now(),
            attempts: 3,
            error: "test error".to_string()
        }
        .is_terminal());
        assert!(JobStatus::Cancelled {
            cancelled_at: Utc::now()
        }
        .is_terminal());
    }

    #[test]
    fn test_status_name() {
        assert_eq!(JobStatus::Pending.name(), "pending");
        assert_eq!(
            JobStatus::Running {
                started_at: Utc::now()
            }
            .name(),
            "running"
        );
        assert_eq!(
            JobStatus::Completed {
                completed_at: Utc::now()
            }
            .name(),
            "completed"
        );
    }

    #[test]
    fn test_status_display() {
        let status = JobStatus::Pending;
        assert_eq!(format!("{status}"), "pending");
    }
}

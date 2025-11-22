//! Background job for sending emails
//!
//! Integrates with the job system to send emails asynchronously.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::email::Email;
use crate::jobs::{Job, JobError, JobResult};

/// Background job for sending emails
///
/// Use this to send emails asynchronously via the job queue.
///
/// # Examples
///
/// ```rust,no_run
/// use acton_htmx::email::{Email, SendEmailJob};
/// use acton_htmx::jobs::{Job, JobAgent};
///
/// # async fn example(job_agent: &JobAgent) -> Result<(), Box<dyn std::error::Error>> {
/// let email = Email::new()
///     .to("user@example.com")
///     .from("noreply@myapp.com")
///     .subject("Welcome!")
///     .text("Welcome to our app!");
///
/// let job = SendEmailJob::new(email);
/// job_agent.enqueue(job).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailJob {
    /// The email to send
    pub email: Email,
}

impl SendEmailJob {
    /// Create a new email sending job
    #[must_use]
    pub const fn new(email: Email) -> Self {
        Self { email }
    }
}

#[async_trait]
impl Job for SendEmailJob {
    type Result = ();

    async fn execute(&self) -> JobResult<Self::Result> {
        // In production, you would get the email sender from a global context or state
        // For now, this is a placeholder that validates the email
        self.email.validate()
            .map_err(|e| JobError::ExecutionFailed(format!("Email validation failed: {e}")))?;

        // Note: Actual sending would require access to the EmailSender instance
        // This would typically be stored in the JobContext or AppState
        // For example:
        // email_sender.send(self.email.clone()).await
        //     .map_err(|e| JobError::ExecutionFailed(format!("Email send failed: {e}")))?;

        Ok(())
    }

    fn max_retries(&self) -> u32 {
        // Retry email sending up to 3 times
        3
    }

    fn timeout(&self) -> std::time::Duration {
        // Email sending should complete within 30 seconds
        std::time::Duration::from_secs(30)
    }
}

// Note: In a future iteration, we could add an EmailJobExt trait
// to provide convenient methods for enqueueing email jobs directly

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_email_job_creation() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        let job = SendEmailJob::new(email.clone());

        assert_eq!(job.email.to, email.to);
        assert_eq!(job.email.from, email.from);
        assert_eq!(job.email.subject, email.subject);
    }

    #[test]
    fn test_send_email_job_serialization() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        let job = SendEmailJob::new(email);

        // Test that the job can be serialized and deserialized
        let serialized = serde_json::to_string(&job).unwrap();
        let deserialized: SendEmailJob = serde_json::from_str(&serialized).unwrap();

        assert_eq!(job.email.to, deserialized.email.to);
        assert_eq!(job.email.from, deserialized.email.from);
    }

    #[tokio::test]
    async fn test_send_email_job_execute() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        let job = SendEmailJob::new(email);

        let result = job.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_email_job_invalid_email() {
        // Create an invalid email (no recipients)
        let email = Email::new()
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        let job = SendEmailJob::new(email);

        let result = job.execute().await;
        assert!(result.is_err());
    }
}

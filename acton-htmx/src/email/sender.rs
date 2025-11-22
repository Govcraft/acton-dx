//! Email sender trait abstraction
//!
//! This module defines the core `EmailSender` trait that all email backends implement.

use async_trait::async_trait;

use super::{Email, EmailError};

/// Trait for sending emails
///
/// Implemented by all email backends (SMTP, AWS SES, console, etc.)
///
/// # Examples
///
/// ```rust,no_run
/// use acton_htmx::email::{Email, EmailSender, SmtpBackend};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let sender = SmtpBackend::from_env()?;
///
/// let email = Email::new()
///     .to("user@example.com")
///     .from("noreply@myapp.com")
///     .subject("Hello!")
///     .text("Hello, World!");
///
/// sender.send(email).await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Send an email
    ///
    /// # Errors
    ///
    /// Returns `EmailError` if the email cannot be sent or is invalid
    async fn send(&self, email: Email) -> Result<(), EmailError>;

    /// Send multiple emails in batch
    ///
    /// Default implementation sends emails sequentially. Backends can override
    /// for more efficient batch sending.
    ///
    /// # Errors
    ///
    /// Returns `EmailError` if any email fails to send
    async fn send_batch(&self, emails: Vec<Email>) -> Result<(), EmailError> {
        for email in emails {
            self.send(email).await?;
        }
        Ok(())
    }
}

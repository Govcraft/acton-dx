//! Testing utilities for email system
//!
//! Provides mock email senders and assertion helpers for testing.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::{Email, EmailError, EmailSender};

/// Mock email sender for testing
///
/// Captures sent emails in memory for assertions.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::email::{Email, EmailSender, MockEmailSender};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mock = MockEmailSender::new();
///
/// let email = Email::new()
///     .to("user@example.com")
///     .from("noreply@myapp.com")
///     .subject("Test")
///     .text("Hello");
///
/// mock.send(email).await?;
///
/// // Verify email was sent
/// assert_eq!(mock.sent_count(), 1);
/// assert!(mock.was_sent_to("user@example.com"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockEmailSender {
    sent: Arc<Mutex<Vec<Email>>>,
}

impl MockEmailSender {
    /// Create a new mock email sender
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of emails sent
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (should never happen in tests)
    #[must_use]
    pub fn sent_count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }

    /// Get all sent emails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (should never happen in tests)
    #[must_use]
    pub fn sent_emails(&self) -> Vec<Email> {
        self.sent.lock().unwrap().clone()
    }

    /// Clear all sent emails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (should never happen in tests)
    pub fn clear(&self) {
        self.sent.lock().unwrap().clear();
    }

    /// Check if an email was sent to a specific address
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (should never happen in tests)
    #[must_use]
    pub fn was_sent_to(&self, address: &str) -> bool {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .any(|email| email.to.contains(&address.to_string()))
    }

    /// Check if an email was sent with a specific subject
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (should never happen in tests)
    #[must_use]
    pub fn was_sent_with_subject(&self, subject: &str) -> bool {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .any(|email| email.subject.as_deref() == Some(subject))
    }

    /// Get the last sent email
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (should never happen in tests)
    #[must_use]
    pub fn last_sent(&self) -> Option<Email> {
        self.sent.lock().unwrap().last().cloned()
    }

    /// Get the first sent email
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned (should never happen in tests)
    #[must_use]
    pub fn first_sent(&self) -> Option<Email> {
        self.sent.lock().unwrap().first().cloned()
    }
}

#[async_trait]
impl EmailSender for MockEmailSender {
    async fn send(&self, email: Email) -> Result<(), EmailError> {
        // Validate email before recording
        email.validate()?;

        // Record sent email
        self.sent.lock().unwrap().push(email);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_email_sender() {
        let mock = MockEmailSender::new();

        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        mock.send(email).await.unwrap();

        assert_eq!(mock.sent_count(), 1);
        assert!(mock.was_sent_to("user@example.com"));
        assert!(mock.was_sent_with_subject("Test"));
    }

    #[tokio::test]
    async fn test_mock_email_sender_multiple() {
        let mock = MockEmailSender::new();

        for i in 0..5 {
            let email = Email::new()
                .to(&format!("user{i}@example.com"))
                .from("noreply@myapp.com")
                .subject(&format!("Test {i}"))
                .text("Hello");

            mock.send(email).await.unwrap();
        }

        assert_eq!(mock.sent_count(), 5);
        assert!(mock.was_sent_to("user0@example.com"));
        assert!(mock.was_sent_to("user4@example.com"));
        assert!(mock.was_sent_with_subject("Test 0"));
        assert!(mock.was_sent_with_subject("Test 4"));
    }

    #[tokio::test]
    async fn test_mock_email_sender_clear() {
        let mock = MockEmailSender::new();

        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        mock.send(email).await.unwrap();
        assert_eq!(mock.sent_count(), 1);

        mock.clear();
        assert_eq!(mock.sent_count(), 0);
    }

    #[tokio::test]
    async fn test_mock_email_sender_last_sent() {
        let mock = MockEmailSender::new();

        let email1 = Email::new()
            .to("user1@example.com")
            .from("noreply@myapp.com")
            .subject("First")
            .text("Hello");

        let email2 = Email::new()
            .to("user2@example.com")
            .from("noreply@myapp.com")
            .subject("Second")
            .text("Hello");

        mock.send(email1).await.unwrap();
        mock.send(email2).await.unwrap();

        let last = mock.last_sent().unwrap();
        assert_eq!(last.subject, Some("Second".to_string()));
        assert_eq!(last.to, vec!["user2@example.com"]);
    }

    #[tokio::test]
    async fn test_mock_email_sender_first_sent() {
        let mock = MockEmailSender::new();

        let email1 = Email::new()
            .to("user1@example.com")
            .from("noreply@myapp.com")
            .subject("First")
            .text("Hello");

        let email2 = Email::new()
            .to("user2@example.com")
            .from("noreply@myapp.com")
            .subject("Second")
            .text("Hello");

        mock.send(email1).await.unwrap();
        mock.send(email2).await.unwrap();

        let first = mock.first_sent().unwrap();
        assert_eq!(first.subject, Some("First".to_string()));
        assert_eq!(first.to, vec!["user1@example.com"]);
    }

    #[tokio::test]
    async fn test_mock_email_sender_invalid_email() {
        let mock = MockEmailSender::new();

        // Create invalid email (no recipients)
        let email = Email::new()
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        let result = mock.send(email).await;
        assert!(result.is_err());
        assert_eq!(mock.sent_count(), 0); // Should not record invalid emails
    }
}

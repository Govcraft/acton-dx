//! Microservices email backend
//!
//! Sends emails via the email-service gRPC endpoint instead of directly
//! through SMTP or other local backends.

#[cfg(feature = "microservices")]
use crate::htmx::clients::{EmailClient, EmailMessage, ServiceRegistry};
#[cfg(feature = "microservices")]
use crate::htmx::email::{Email, EmailError, EmailSender};
#[cfg(feature = "microservices")]
use async_trait::async_trait;
#[cfg(feature = "microservices")]
use std::sync::Arc;
#[cfg(feature = "microservices")]
use tokio::sync::RwLock;

/// Email backend that sends via the email microservice
///
/// Uses the email-service gRPC endpoint for sending emails. This allows
/// centralized email sending with features like queuing, rate limiting,
/// and templates handled by the service.
///
/// # Example
///
/// ```rust,no_run
/// use acton_htmx::email::{Email, EmailSender};
/// use acton_htmx::email::MicroservicesEmailBackend;
/// use acton_htmx::clients::{ServiceRegistry, ServicesConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ServicesConfig {
///     email_endpoint: Some("http://localhost:50056".to_string()),
///     ..Default::default()
/// };
/// let registry = ServiceRegistry::from_config(&config).await?;
/// let backend = MicroservicesEmailBackend::new(&registry)?;
///
/// let email = Email::new()
///     .to("user@example.com")
///     .from("noreply@myapp.com")
///     .subject("Hello!")
///     .text("Hello, World!");
///
/// backend.send(email).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "microservices")]
#[derive(Clone)]
pub struct MicroservicesEmailBackend {
    client: Arc<RwLock<EmailClient>>,
}

#[cfg(feature = "microservices")]
impl std::fmt::Debug for MicroservicesEmailBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroservicesEmailBackend")
            .field("client", &"EmailClient")
            .finish()
    }
}

#[cfg(feature = "microservices")]
impl MicroservicesEmailBackend {
    /// Create a new microservices email backend from a service registry
    ///
    /// # Errors
    ///
    /// Returns error if the email service is not configured in the registry.
    pub fn new(registry: &ServiceRegistry) -> Result<Self, crate::htmx::clients::ClientError> {
        let client = registry.email()?;
        Ok(Self { client })
    }

    /// Create from an existing email client
    #[must_use]
    pub const fn from_client(client: Arc<RwLock<EmailClient>>) -> Self {
        Self { client }
    }
}

#[cfg(feature = "microservices")]
#[async_trait]
impl EmailSender for MicroservicesEmailBackend {
    async fn send(&self, email: Email) -> Result<(), EmailError> {
        // Validate email before sending
        email.validate()?;

        // Convert to client EmailMessage
        let message = email_to_message(&email);

        // Send via gRPC client and immediately drop the guard
        let result = {
            let mut client = self.client.write().await;
            client
                .send(message)
                .await
                .map_err(|e| EmailError::service(format!("service error: {e}")))?
        };

        if result.success {
            Ok(())
        } else {
            Err(EmailError::service(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn send_batch(&self, emails: Vec<Email>) -> Result<(), EmailError> {
        // Validate all emails before sending
        for email in &emails {
            email.validate()?;
        }

        // Convert to client EmailMessages
        let messages: Vec<EmailMessage> = emails.iter().map(email_to_message).collect();

        // Send batch via gRPC client and immediately drop the guard
        let result = {
            let mut client = self.client.write().await;
            client
                .send_batch(messages)
                .await
                .map_err(|e| EmailError::service(format!("service error: {e}")))?
        };

        if result.failed > 0 {
            // Collect error messages from failed sends
            let errors: Vec<String> = result
                .results
                .iter()
                .filter(|r| !r.success)
                .filter_map(|r| r.error.clone())
                .collect();

            Err(EmailError::service(format!(
                "{} of {} emails failed: {}",
                result.failed,
                result.total,
                errors.join(", ")
            )))
        } else {
            Ok(())
        }
    }
}

/// Convert a local Email to a client EmailMessage
#[cfg(feature = "microservices")]
fn email_to_message(email: &Email) -> EmailMessage {
    use crate::htmx::clients::EmailAddr;

    let mut message = EmailMessage::new();

    // Set from address
    if let Some(ref from) = email.from {
        message.from = EmailAddr {
            email: from.clone(),
            name: None,
        };
    }

    // Set recipients
    for to in &email.to {
        message.to.push(EmailAddr {
            email: to.clone(),
            name: None,
        });
    }

    // Set CC recipients
    for cc in &email.cc {
        message.cc.push(EmailAddr {
            email: cc.clone(),
            name: None,
        });
    }

    // Set BCC recipients
    for bcc in &email.bcc {
        message.bcc.push(EmailAddr {
            email: bcc.clone(),
            name: None,
        });
    }

    // Set reply-to
    if let Some(ref reply_to) = email.reply_to {
        message.reply_to = Some(EmailAddr {
            email: reply_to.clone(),
            name: None,
        });
    }

    // Set subject
    if let Some(ref subject) = email.subject {
        message.subject.clone_from(subject);
    }

    // Set body content
    message.text_body.clone_from(&email.text);
    message.html_body.clone_from(&email.html);

    // Set headers
    for (key, value) in &email.headers {
        message.headers.insert(key.clone(), value.clone());
    }

    message
}

#[cfg(all(test, feature = "microservices"))]
mod tests {
    use super::*;

    #[test]
    fn test_email_to_message_conversion() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test Subject")
            .text("Plain text body")
            .html("<p>HTML body</p>");

        let message = email_to_message(&email);

        assert_eq!(message.from.email, "noreply@myapp.com");
        assert_eq!(message.to.len(), 1);
        assert_eq!(message.to[0].email, "user@example.com");
        assert_eq!(message.subject, "Test Subject");
        assert_eq!(message.text_body, Some("Plain text body".to_string()));
        assert_eq!(message.html_body, Some("<p>HTML body</p>".to_string()));
    }

    #[test]
    fn test_email_to_message_multiple_recipients() {
        let email = Email::new()
            .to("user1@example.com")
            .to("user2@example.com")
            .cc("cc@example.com")
            .bcc("bcc@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Body");

        let message = email_to_message(&email);

        assert_eq!(message.to.len(), 2);
        assert_eq!(message.cc.len(), 1);
        assert_eq!(message.bcc.len(), 1);
        assert_eq!(message.cc[0].email, "cc@example.com");
        assert_eq!(message.bcc[0].email, "bcc@example.com");
    }

    #[test]
    fn test_email_to_message_with_headers() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Body")
            .header("X-Custom", "value")
            .header("X-Priority", "1");

        let message = email_to_message(&email);

        assert_eq!(message.headers.len(), 2);
        assert_eq!(message.headers.get("X-Custom"), Some(&"value".to_string()));
        assert_eq!(message.headers.get("X-Priority"), Some(&"1".to_string()));
    }
}

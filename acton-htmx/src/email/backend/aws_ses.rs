//! AWS SES backend for sending emails
//!
//! Uses AWS Simple Email Service (SES) v2 API for sending emails.
//! Requires the `aws-sdk-sesv2` feature to be enabled.

#[cfg(feature = "aws-sdk-sesv2")]
use async_trait::async_trait;
#[cfg(feature = "aws-sdk-sesv2")]
use aws_sdk_sesv2::{
    types::{Body, Content, Destination, EmailContent, Message},
    Client,
};

use crate::email::{Email, EmailError, EmailSender};

/// AWS SES email backend
///
/// Sends emails via Amazon Simple Email Service (SES).
///
/// Requires the `aws-sdk-sesv2` feature to be enabled.
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(feature = "aws-sdk-sesv2")]
/// # {
/// use acton_htmx::email::{Email, EmailSender, AwsSesBackend};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create backend (uses AWS SDK default credential chain)
/// let backend = AwsSesBackend::from_env().await?;
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
/// # }
/// ```
#[cfg(feature = "aws-sdk-sesv2")]
pub struct AwsSesBackend {
    client: Client,
}

#[cfg(feature = "aws-sdk-sesv2")]
impl AwsSesBackend {
    /// Create a new AWS SES backend with the given client
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Create a new AWS SES backend using the default AWS SDK configuration
    ///
    /// This uses the default credential provider chain, which checks:
    /// 1. Environment variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
    /// 2. AWS credentials file (~/.aws/credentials)
    /// 3. IAM instance profile (when running on EC2)
    ///
    /// # Errors
    ///
    /// Returns `EmailError::ConfigError` if AWS SDK configuration fails
    pub async fn from_env() -> Result<Self, EmailError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .load()
            .await;
        let client = Client::new(&config);
        Ok(Self::new(client))
    }

    /// Build AWS SES message from Email
    fn build_message(&self, email: &Email) -> Result<EmailContent, EmailError> {
        // Validate email first
        email.validate()?;

        // Build destination
        let mut destination = Destination::builder();
        for to_addr in &email.to {
            destination = destination.to_addresses(to_addr);
        }
        for cc_addr in &email.cc {
            destination = destination.cc_addresses(cc_addr);
        }
        for bcc_addr in &email.bcc {
            destination = destination.bcc_addresses(bcc_addr);
        }

        // Build subject
        let subject = email.subject.as_ref().ok_or(EmailError::NoSubject)?;
        let subject_content = Content::builder().data(subject).build().map_err(|e| {
            EmailError::aws_ses(format!("Failed to build subject content: {e}"))
        })?;

        // Build body
        let body = if let (Some(html), Some(text)) = (&email.html, &email.text) {
            // Both HTML and text
            let html_content = Content::builder().data(html).build().map_err(|e| {
                EmailError::aws_ses(format!("Failed to build HTML content: {e}"))
            })?;
            let text_content = Content::builder().data(text).build().map_err(|e| {
                EmailError::aws_ses(format!("Failed to build text content: {e}"))
            })?;
            Body::builder()
                .html(html_content)
                .text(text_content)
                .build()
        } else if let Some(html) = &email.html {
            // HTML only
            let html_content = Content::builder().data(html).build().map_err(|e| {
                EmailError::aws_ses(format!("Failed to build HTML content: {e}"))
            })?;
            Body::builder().html(html_content).build()
        } else if let Some(text) = &email.text {
            // Text only
            let text_content = Content::builder().data(text).build().map_err(|e| {
                EmailError::aws_ses(format!("Failed to build text content: {e}"))
            })?;
            Body::builder().text(text_content).build()
        } else {
            return Err(EmailError::NoContent);
        };

        // Build message
        let message = Message::builder()
            .subject(subject_content)
            .body(body)
            .build()
            .map_err(|e| EmailError::aws_ses(format!("Failed to build message: {e}")))?;

        Ok(EmailContent::builder().simple(message).build())
    }
}

#[cfg(feature = "aws-sdk-sesv2")]
#[async_trait]
impl EmailSender for AwsSesBackend {
    async fn send(&self, email: Email) -> Result<(), EmailError> {
        let from_addr = email.from.as_ref().ok_or(EmailError::NoSender)?;
        let content = self.build_message(&email)?;

        let mut request = self
            .client
            .send_email()
            .from_email_address(from_addr)
            .content(content);

        // Add reply-to if present
        if let Some(reply_to) = &email.reply_to {
            request = request.reply_to_addresses(reply_to);
        }

        // Send the email
        request
            .send()
            .await
            .map_err(|e| EmailError::aws_ses(format!("Failed to send email: {e}")))?;

        Ok(())
    }
}

/// Stub implementation when AWS SES feature is not enabled
///
/// This struct is only available when the `aws-sdk-sesv2` feature is disabled.
/// Enable the feature to use the full AWS SES backend implementation.
#[cfg(not(feature = "aws-sdk-sesv2"))]
pub struct AwsSesBackend;

#[cfg(not(feature = "aws-sdk-sesv2"))]
impl AwsSesBackend {
    /// AWS SES backend is not available without the `aws-sdk-sesv2` feature
    ///
    /// # Errors
    ///
    /// Always returns an error indicating the feature is not enabled
    #[allow(clippy::unused_async)]
    pub async fn from_env() -> Result<Self, EmailError> {
        Err(EmailError::config(
            "AWS SES backend requires the 'aws-sdk-sesv2' feature to be enabled",
        ))
    }
}

#[cfg(not(feature = "aws-sdk-sesv2"))]
#[async_trait::async_trait]
impl EmailSender for AwsSesBackend {
    async fn send(&self, _email: Email) -> Result<(), EmailError> {
        Err(EmailError::config(
            "AWS SES backend requires the 'aws-sdk-sesv2' feature to be enabled",
        ))
    }
}

#[cfg(all(test, feature = "aws-sdk-sesv2"))]
mod tests {
    use super::*;

    #[test]
    fn test_build_message_simple() {
        let email = Email::new()
            .to("recipient@example.com")
            .from("sender@example.com")
            .subject("Test Email")
            .text("This is a test email");

        let config = aws_sdk_sesv2::Config::builder().build();
        let client = Client::from_conf(config);
        let backend = AwsSesBackend::new(client);

        let content = backend.build_message(&email);
        assert!(content.is_ok());
    }

    #[test]
    fn test_build_message_with_html_and_text() {
        let email = Email::new()
            .to("recipient@example.com")
            .from("sender@example.com")
            .subject("Test Email")
            .text("This is plain text")
            .html("<h1>This is HTML</h1>");

        let config = aws_sdk_sesv2::Config::builder().build();
        let client = Client::from_conf(config);
        let backend = AwsSesBackend::new(client);

        let content = backend.build_message(&email);
        assert!(content.is_ok());
    }
}

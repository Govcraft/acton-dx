//! Email service client for sending emails.

use super::error::ClientError;
use acton_dx_proto::email::v1::{
    email_service_client::EmailServiceClient, Attachment, Email, EmailAddress, SendBatchRequest,
    SendEmailRequest, ValidateAddressRequest,
};
use tonic::transport::Channel;

/// Client for the email service.
///
/// Provides email sending with support for attachments and batch operations.
#[derive(Debug, Clone)]
pub struct EmailClient {
    client: EmailServiceClient<Channel>,
}

impl EmailClient {
    /// Connect to the email service.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, ClientError> {
        let endpoint = endpoint.into();
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| ClientError::ConnectionFailed(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            client: EmailServiceClient::new(channel),
        })
    }

    /// Send a single email.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn send(&mut self, email: EmailMessage) -> Result<SendResult, ClientError> {
        let proto_email = email.into_proto();
        let response = self
            .client
            .send_email(SendEmailRequest {
                email: Some(proto_email),
            })
            .await?;

        let inner = response.into_inner();
        Ok(SendResult {
            success: inner.success,
            message_id: inner.message_id,
            error: inner.error,
        })
    }

    /// Send multiple emails in batch.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn send_batch(
        &mut self,
        emails: Vec<EmailMessage>,
    ) -> Result<BatchSendResult, ClientError> {
        let proto_emails: Vec<Email> = emails.into_iter().map(EmailMessage::into_proto).collect();
        let response = self
            .client
            .send_batch(SendBatchRequest {
                emails: proto_emails,
            })
            .await?;

        let inner = response.into_inner();
        Ok(BatchSendResult {
            total: inner.total,
            succeeded: inner.succeeded,
            failed: inner.failed,
            results: inner
                .results
                .into_iter()
                .map(|r| SendResult {
                    success: r.success,
                    message_id: r.message_id,
                    error: r.error,
                })
                .collect(),
        })
    }

    /// Validate an email address.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn validate_address(&mut self, email: &str) -> Result<ValidationResult, ClientError> {
        let response = self
            .client
            .validate_address(ValidateAddressRequest {
                email: email.to_string(),
            })
            .await?;

        let inner = response.into_inner();
        Ok(ValidationResult {
            valid: inner.valid,
            reason: inner.reason,
        })
    }
}

/// An email message to send.
#[derive(Debug, Clone, Default)]
pub struct EmailMessage {
    /// Sender address.
    pub from: EmailAddr,
    /// Primary recipients.
    pub to: Vec<EmailAddr>,
    /// Carbon copy recipients.
    pub cc: Vec<EmailAddr>,
    /// Blind carbon copy recipients.
    pub bcc: Vec<EmailAddr>,
    /// Reply-to address.
    pub reply_to: Option<EmailAddr>,
    /// Email subject.
    pub subject: String,
    /// Plain text body.
    pub text_body: Option<String>,
    /// HTML body.
    pub html_body: Option<String>,
    /// Attachments.
    pub attachments: Vec<EmailAttachment>,
    /// Additional headers.
    pub headers: std::collections::HashMap<String, String>,
}

impl EmailMessage {
    /// Create a new email message.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the from address.
    #[must_use]
    pub fn from(mut self, email: impl Into<String>) -> Self {
        self.from = EmailAddr {
            email: email.into(),
            name: None,
        };
        self
    }

    /// Set the from address with a name.
    #[must_use]
    pub fn from_named(mut self, email: impl Into<String>, name: impl Into<String>) -> Self {
        self.from = EmailAddr {
            email: email.into(),
            name: Some(name.into()),
        };
        self
    }

    /// Add a recipient.
    #[must_use]
    pub fn to(mut self, email: impl Into<String>) -> Self {
        self.to.push(EmailAddr {
            email: email.into(),
            name: None,
        });
        self
    }

    /// Add a recipient with a name.
    #[must_use]
    pub fn to_named(mut self, email: impl Into<String>, name: impl Into<String>) -> Self {
        self.to.push(EmailAddr {
            email: email.into(),
            name: Some(name.into()),
        });
        self
    }

    /// Set the subject.
    #[must_use]
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = subject.into();
        self
    }

    /// Set the plain text body.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text_body = Some(text.into());
        self
    }

    /// Set the HTML body.
    #[must_use]
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html_body = Some(html.into());
        self
    }

    /// Add an attachment.
    #[must_use]
    pub fn attach(mut self, filename: impl Into<String>, content: Vec<u8>, content_type: impl Into<String>) -> Self {
        self.attachments.push(EmailAttachment {
            filename: filename.into(),
            content,
            content_type: content_type.into(),
        });
        self
    }

    /// Convert to proto message.
    fn into_proto(self) -> Email {
        Email {
            from: Some(self.from.into_proto()),
            to: self.to.into_iter().map(EmailAddr::into_proto).collect(),
            cc: self.cc.into_iter().map(EmailAddr::into_proto).collect(),
            bcc: self.bcc.into_iter().map(EmailAddr::into_proto).collect(),
            reply_to: self.reply_to.map(EmailAddr::into_proto),
            subject: self.subject,
            text_body: self.text_body,
            html_body: self.html_body,
            attachments: self
                .attachments
                .into_iter()
                .map(EmailAttachment::into_proto)
                .collect(),
            headers: self.headers,
        }
    }
}

/// An email address with optional display name.
#[derive(Debug, Clone, Default)]
pub struct EmailAddr {
    /// Email address.
    pub email: String,
    /// Display name.
    pub name: Option<String>,
}

impl EmailAddr {
    fn into_proto(self) -> EmailAddress {
        EmailAddress {
            email: self.email,
            name: self.name,
        }
    }
}

/// An email attachment.
#[derive(Debug, Clone)]
pub struct EmailAttachment {
    /// Filename.
    pub filename: String,
    /// File content.
    pub content: Vec<u8>,
    /// MIME content type.
    pub content_type: String,
}

impl EmailAttachment {
    fn into_proto(self) -> Attachment {
        Attachment {
            filename: self.filename,
            content: self.content,
            content_type: self.content_type,
        }
    }
}

/// Result of sending a single email.
#[derive(Debug, Clone)]
pub struct SendResult {
    /// Whether the send succeeded.
    pub success: bool,
    /// Message ID from the mail server.
    pub message_id: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Result of sending a batch of emails.
#[derive(Debug, Clone)]
pub struct BatchSendResult {
    /// Total emails in batch.
    pub total: i32,
    /// Number that succeeded.
    pub succeeded: i32,
    /// Number that failed.
    pub failed: i32,
    /// Individual results.
    pub results: Vec<SendResult>,
}

/// Result of email address validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the address is valid.
    pub valid: bool,
    /// Reason if invalid.
    pub reason: Option<String>,
}

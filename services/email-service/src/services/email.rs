//! Email service gRPC implementation.

use acton_dx_proto::email::v1::{
    email_service_server::EmailService, Attachment, Email, EmailAddress, SendBatchRequest,
    SendBatchResponse, SendEmailRequest, SendEmailResponse, ValidateAddressRequest,
    ValidateAddressResponse,
};
use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

/// Internal error type to avoid large error sizes.
#[derive(Debug)]
struct EmailError {
    message: String,
}

impl EmailError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Email service implementation.
pub struct EmailServiceImpl {
    /// SMTP transport.
    transport: Arc<AsyncSmtpTransport<Tokio1Executor>>,
    /// Default from address.
    default_from: Option<Mailbox>,
}

impl EmailServiceImpl {
    /// Create a new email service with SMTP transport.
    ///
    /// # Errors
    ///
    /// Returns error if SMTP transport cannot be created.
    pub fn new(
        host: &str,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
        tls: bool,
        default_from: Option<Mailbox>,
    ) -> anyhow::Result<Self> {
        let mut transport_builder = if tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
        };

        transport_builder = transport_builder.port(port);

        if let (Some(user), Some(pass)) = (username, password) {
            let credentials = Credentials::new(user.to_string(), pass.to_string());
            transport_builder = transport_builder.credentials(credentials);
        }

        let transport = transport_builder.build();

        info!(host = %host, port = %port, tls = %tls, "Created SMTP transport");

        Ok(Self {
            transport: Arc::new(transport),
            default_from,
        })
    }

    /// Create a service for testing (no actual sending).
    #[must_use]
    pub fn mock() -> Self {
        // Use localhost as a placeholder - won't actually connect
        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("localhost")
            .port(25)
            .build();

        Self {
            transport: Arc::new(transport),
            default_from: None,
        }
    }

    /// Convert proto `EmailAddress` to lettre `Mailbox`.
    fn to_mailbox(addr: &EmailAddress) -> Result<Mailbox, EmailError> {
        let email = addr.email.parse().map_err(|e| {
            error!(error = %e, email = %addr.email, "Invalid email address");
            EmailError::new(format!("Invalid email address: {e}"))
        })?;

        if let Some(ref name) = addr.name {
            Ok(Mailbox::new(Some(name.clone()), email))
        } else {
            Ok(Mailbox::new(None, email))
        }
    }

    /// Build a lettre Message from proto Email.
    fn build_message(&self, email: &Email) -> Result<Message, EmailError> {
        // Get from address
        let from = if let Some(ref from_addr) = email.from {
            Self::to_mailbox(from_addr)?
        } else if let Some(ref default) = self.default_from {
            default.clone()
        } else {
            return Err(EmailError::new("Missing 'from' address"));
        };

        // Build message
        let mut builder = Message::builder().from(from);

        // Add recipients
        for to in &email.to {
            builder = builder.to(Self::to_mailbox(to)?);
        }

        for cc in &email.cc {
            builder = builder.cc(Self::to_mailbox(cc)?);
        }

        for bcc in &email.bcc {
            builder = builder.bcc(Self::to_mailbox(bcc)?);
        }

        if let Some(ref reply_to) = email.reply_to {
            builder = builder.reply_to(Self::to_mailbox(reply_to)?);
        }

        builder = builder.subject(&email.subject);

        // Build body
        let message = match (&email.text_body, &email.html_body) {
            (Some(text), Some(html)) => {
                // Multi-part with both text and HTML
                let multipart = MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.clone()),
                    );

                if email.attachments.is_empty() {
                    builder.multipart(multipart)
                } else {
                    let mut mixed = MultiPart::mixed().multipart(multipart);
                    for attachment in &email.attachments {
                        mixed = mixed.singlepart(Self::build_attachment(attachment)?);
                    }
                    builder.multipart(mixed)
                }
            }
            (Some(text), None) => {
                if email.attachments.is_empty() {
                    builder.body(text.clone())
                } else {
                    let mut mixed = MultiPart::mixed().singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text.clone()),
                    );
                    for attachment in &email.attachments {
                        mixed = mixed.singlepart(Self::build_attachment(attachment)?);
                    }
                    builder.multipart(mixed)
                }
            }
            (None, Some(html)) => {
                if email.attachments.is_empty() {
                    builder
                        .header(ContentType::TEXT_HTML)
                        .body(html.clone())
                } else {
                    let mut mixed = MultiPart::mixed().singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html.clone()),
                    );
                    for attachment in &email.attachments {
                        mixed = mixed.singlepart(Self::build_attachment(attachment)?);
                    }
                    builder.multipart(mixed)
                }
            }
            (None, None) => builder.body(String::new()),
        };

        message.map_err(|e| {
            error!(error = %e, "Failed to build email message");
            EmailError::new(format!("Failed to build message: {e}"))
        })
    }

    /// Build an attachment `SinglePart`.
    fn build_attachment(attachment: &Attachment) -> Result<SinglePart, EmailError> {
        let content_type: ContentType = attachment.content_type.parse().map_err(|e| {
            error!(error = %e, "Invalid content type");
            EmailError::new(format!("Invalid content type: {e}"))
        })?;

        Ok(SinglePart::builder()
            .header(content_type)
            .header(lettre::message::header::ContentDisposition::attachment(
                &attachment.filename,
            ))
            .body(attachment.content.clone()))
    }

    /// Send a single email and return the response.
    async fn send_single(&self, email: &Email) -> SendEmailResponse {
        let message = match self.build_message(email) {
            Ok(m) => m,
            Err(e) => {
                return SendEmailResponse {
                    success: false,
                    message_id: None,
                    error: Some(e.message),
                };
            }
        };

        match self.transport.send(message).await {
            Ok(response) => {
                let message_id = uuid::Uuid::new_v4().to_string();
                debug!(message_id = %message_id, "Email sent successfully");
                SendEmailResponse {
                    success: response.is_positive(),
                    message_id: Some(message_id),
                    error: None,
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to send email");
                SendEmailResponse {
                    success: false,
                    message_id: None,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Validate an email address.
    fn validate_email(email: &str) -> (bool, Option<String>) {
        // Basic email validation
        if email.is_empty() {
            return (false, Some("Email address is empty".to_string()));
        }

        if !email.contains('@') {
            return (false, Some("Email address must contain @".to_string()));
        }

        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return (false, Some("Invalid email format".to_string()));
        }

        let local = parts[0];
        let domain = parts[1];

        if local.is_empty() {
            return (false, Some("Local part is empty".to_string()));
        }

        if domain.is_empty() {
            return (false, Some("Domain is empty".to_string()));
        }

        if !domain.contains('.') {
            return (false, Some("Domain must contain a dot".to_string()));
        }

        // Try to parse as email address
        match email.parse::<lettre::Address>() {
            Ok(_) => (true, None),
            Err(e) => (false, Some(e.to_string())),
        }
    }

    /// Safely convert usize to i32.
    fn usize_to_i32(value: usize) -> i32 {
        i32::try_from(value).unwrap_or(i32::MAX)
    }
}

#[tonic::async_trait]
impl EmailService for EmailServiceImpl {
    async fn send_email(
        &self,
        request: Request<SendEmailRequest>,
    ) -> Result<Response<SendEmailResponse>, Status> {
        let req = request.into_inner();

        let email = req
            .email
            .ok_or_else(|| Status::invalid_argument("Missing email"))?;

        let response = self.send_single(&email).await;
        Ok(Response::new(response))
    }

    async fn send_batch(
        &self,
        request: Request<SendBatchRequest>,
    ) -> Result<Response<SendBatchResponse>, Status> {
        let req = request.into_inner();

        let mut results = Vec::with_capacity(req.emails.len());
        let mut succeeded = 0;
        let mut failed = 0;

        for email in &req.emails {
            let result = self.send_single(email).await;
            if result.success {
                succeeded += 1;
            } else {
                failed += 1;
            }
            results.push(result);
        }

        Ok(Response::new(SendBatchResponse {
            total: Self::usize_to_i32(req.emails.len()),
            succeeded,
            failed,
            results,
        }))
    }

    async fn validate_address(
        &self,
        request: Request<ValidateAddressRequest>,
    ) -> Result<Response<ValidateAddressResponse>, Status> {
        let req = request.into_inner();
        let (valid, reason) = Self::validate_email(&req.email);

        Ok(Response::new(ValidateAddressResponse { valid, reason }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email_valid() {
        let (valid, reason) = EmailServiceImpl::validate_email("test@example.com");
        assert!(valid);
        assert!(reason.is_none());
    }

    #[test]
    fn test_validate_email_invalid_no_at() {
        let (valid, reason) = EmailServiceImpl::validate_email("testexample.com");
        assert!(!valid);
        assert!(reason.is_some());
    }

    #[test]
    fn test_validate_email_invalid_empty() {
        let (valid, reason) = EmailServiceImpl::validate_email("");
        assert!(!valid);
        assert_eq!(reason, Some("Email address is empty".to_string()));
    }

    #[test]
    fn test_validate_email_invalid_no_domain() {
        let (valid, reason) = EmailServiceImpl::validate_email("test@");
        assert!(!valid);
        assert!(reason.is_some());
    }

    #[test]
    fn test_safe_conversion() {
        assert_eq!(EmailServiceImpl::usize_to_i32(100), 100);
    }
}

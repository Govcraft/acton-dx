//! SMTP backend for sending emails
//!
//! Uses the `lettre` crate to send emails via SMTP servers.

use async_trait::async_trait;
use lettre::{
    message::{header, Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::email::{Email, EmailError, EmailSender};

/// SMTP email backend configuration
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    /// SMTP server hostname
    pub host: String,

    /// SMTP server port (usually 587 for STARTTLS, 465 for TLS)
    pub port: u16,

    /// SMTP username
    pub username: String,

    /// SMTP password
    pub password: String,

    /// Use STARTTLS (default: true)
    pub use_tls: bool,
}

impl SmtpConfig {
    /// Create SMTP configuration from environment variables
    ///
    /// Expects the following environment variables:
    /// - `SMTP_HOST`: SMTP server hostname
    /// - `SMTP_PORT`: SMTP server port (default: 587)
    /// - `SMTP_USERNAME`: SMTP username
    /// - `SMTP_PASSWORD`: SMTP password
    /// - `SMTP_USE_TLS`: Use TLS (default: true)
    ///
    /// # Errors
    ///
    /// Returns `EmailError::ConfigError` if required environment variables are missing
    pub fn from_env() -> Result<Self, EmailError> {
        let host = std::env::var("SMTP_HOST")
            .map_err(|_| EmailError::config("SMTP_HOST environment variable not set"))?;

        let port = std::env::var("SMTP_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()
            .map_err(|_| EmailError::config("SMTP_PORT must be a valid port number"))?;

        let username = std::env::var("SMTP_USERNAME")
            .map_err(|_| EmailError::config("SMTP_USERNAME environment variable not set"))?;

        let password = std::env::var("SMTP_PASSWORD")
            .map_err(|_| EmailError::config("SMTP_PASSWORD environment variable not set"))?;

        let use_tls = std::env::var("SMTP_USE_TLS")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);

        Ok(Self {
            host,
            port,
            username,
            password,
            use_tls,
        })
    }
}

/// SMTP email backend
///
/// Sends emails via SMTP using the `lettre` crate.
///
/// # Examples
///
/// ```rust,no_run
/// use acton_htmx::email::{Email, EmailSender, SmtpBackend};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create backend from environment variables
/// let backend = SmtpBackend::from_env()?;
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
pub struct SmtpBackend {
    config: SmtpConfig,
}

impl SmtpBackend {
    /// Create a new SMTP backend with the given configuration
    #[must_use]
    pub const fn new(config: SmtpConfig) -> Self {
        Self { config }
    }

    /// Create a new SMTP backend from environment variables
    ///
    /// # Errors
    ///
    /// Returns `EmailError::ConfigError` if required environment variables are missing
    pub fn from_env() -> Result<Self, EmailError> {
        let config = SmtpConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Build lettre Message from Email
    fn build_message(email: &Email) -> Result<Message, EmailError> {
        // Validate email first
        email.validate()?;

        let from_addr = email.from.as_ref().ok_or(EmailError::NoSender)?;
        let from: Mailbox = from_addr
            .parse()
            .map_err(|_| EmailError::InvalidAddress(from_addr.clone()))?;

        // Start building message
        let mut builder = Message::builder().from(from);

        // Add recipients
        for to_addr in &email.to {
            let to: Mailbox = to_addr
                .parse()
                .map_err(|_| EmailError::InvalidAddress(to_addr.clone()))?;
            builder = builder.to(to);
        }

        // Add CC recipients
        for cc_addr in &email.cc {
            let cc: Mailbox = cc_addr
                .parse()
                .map_err(|_| EmailError::InvalidAddress(cc_addr.clone()))?;
            builder = builder.cc(cc);
        }

        // Add BCC recipients
        for bcc_addr in &email.bcc {
            let bcc: Mailbox = bcc_addr
                .parse()
                .map_err(|_| EmailError::InvalidAddress(bcc_addr.clone()))?;
            builder = builder.bcc(bcc);
        }

        // Add Reply-To
        if let Some(reply_to_addr) = &email.reply_to {
            let reply_to: Mailbox = reply_to_addr
                .parse()
                .map_err(|_| EmailError::InvalidAddress(reply_to_addr.clone()))?;
            builder = builder.reply_to(reply_to);
        }

        // Add subject
        let subject = email.subject.as_ref().ok_or(EmailError::NoSubject)?;
        builder = builder.subject(subject);

        // Note: Custom headers would require using specific lettre header types
        // For now, we skip custom headers as they require more complex implementation
        // Future enhancement: Add support for common headers like X-Priority

        // Build multipart message if we have both HTML and text
        let message = if let (Some(html), Some(text)) = (&email.html, &email.text) {
            builder
                .multipart(
                    MultiPart::alternative()
                        .singlepart(
                            SinglePart::builder()
                                .header(header::ContentType::TEXT_PLAIN)
                                .body(text.clone()),
                        )
                        .singlepart(
                            SinglePart::builder()
                                .header(header::ContentType::TEXT_HTML)
                                .body(html.clone()),
                        ),
                )
                .map_err(|e| EmailError::smtp(e.to_string()))?
        } else if let Some(html) = &email.html {
            builder
                .header(header::ContentType::TEXT_HTML)
                .body(html.clone())
                .map_err(|e| EmailError::smtp(e.to_string()))?
        } else if let Some(text) = &email.text {
            builder
                .header(header::ContentType::TEXT_PLAIN)
                .body(text.clone())
                .map_err(|e| EmailError::smtp(e.to_string()))?
        } else {
            return Err(EmailError::NoContent);
        };

        Ok(message)
    }

    /// Create SMTP transport from config
    fn create_transport(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>, EmailError> {
        let credentials = Credentials::new(
            self.config.username.clone(),
            self.config.password.clone(),
        );

        let mut transport = if self.config.use_tls {
            let tls_parameters = TlsParameters::new(self.config.host.clone())
                .map_err(|e| EmailError::smtp(format!("TLS parameters error: {e}")))?;

            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                .map_err(|e| EmailError::smtp(e.to_string()))?
                .credentials(credentials)
                .tls(Tls::Required(tls_parameters))
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                .credentials(credentials)
        };

        transport = transport.port(self.config.port);

        Ok(transport.build())
    }
}

#[async_trait]
impl EmailSender for SmtpBackend {
    async fn send(&self, email: Email) -> Result<(), EmailError> {
        let message = Self::build_message(&email)?;
        let transport = self.create_transport()?;

        transport
            .send(message)
            .await
            .map_err(|e| EmailError::smtp(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smtp_config_from_env() {
        std::env::set_var("SMTP_HOST", "smtp.example.com");
        std::env::set_var("SMTP_PORT", "587");
        std::env::set_var("SMTP_USERNAME", "user@example.com");
        std::env::set_var("SMTP_PASSWORD", "password123");
        std::env::set_var("SMTP_USE_TLS", "true");

        let config = SmtpConfig::from_env().unwrap();

        assert_eq!(config.host, "smtp.example.com");
        assert_eq!(config.port, 587);
        assert_eq!(config.username, "user@example.com");
        assert_eq!(config.password, "password123");
        assert!(config.use_tls);
    }

    #[test]
    fn test_smtp_config_defaults() {
        std::env::remove_var("SMTP_PORT");
        std::env::remove_var("SMTP_USE_TLS");
        std::env::set_var("SMTP_HOST", "smtp.example.com");
        std::env::set_var("SMTP_USERNAME", "user@example.com");
        std::env::set_var("SMTP_PASSWORD", "password123");

        let config = SmtpConfig::from_env().unwrap();

        assert_eq!(config.port, 587); // default
        assert!(config.use_tls); // default
    }

    #[test]
    fn test_build_message_simple() {
        let email = Email::new()
            .to("recipient@example.com")
            .from("sender@example.com")
            .subject("Test Email")
            .text("This is a test email");

        let message = SmtpBackend::build_message(&email);
        assert!(message.is_ok());
    }

    #[test]
    fn test_build_message_with_html_and_text() {
        let email = Email::new()
            .to("recipient@example.com")
            .from("sender@example.com")
            .subject("Test Email")
            .text("This is plain text")
            .html("<h1>This is HTML</h1>");

        let message = SmtpBackend::build_message(&email);
        assert!(message.is_ok());
    }

    #[test]
    fn test_build_message_with_cc_and_bcc() {
        let email = Email::new()
            .to("recipient@example.com")
            .cc("cc@example.com")
            .bcc("bcc@example.com")
            .from("sender@example.com")
            .subject("Test Email")
            .text("Test content");

        let message = SmtpBackend::build_message(&email);
        assert!(message.is_ok());
    }
}

//! Email error types

use thiserror::Error;

/// Errors that can occur when working with emails
#[derive(Debug, Error)]
pub enum EmailError {
    /// Email has no recipients
    #[error("email must have at least one recipient")]
    NoRecipients,

    /// Email has no sender
    #[error("email must have a from address")]
    NoSender,

    /// Email has no subject
    #[error("email must have a subject")]
    NoSubject,

    /// Email has no body content
    #[error("email must have either text or HTML content")]
    NoContent,

    /// Invalid email address format
    #[error("invalid email address: {0}")]
    InvalidAddress(String),

    /// Template rendering error
    #[error("failed to render email template: {0}")]
    TemplateError(#[from] askama::Error),

    /// SMTP transport error
    #[error("SMTP error: {0}")]
    SmtpError(String),

    /// AWS SES error
    #[error("AWS SES error: {0}")]
    AwsSesError(String),

    /// Email configuration error
    #[error("email configuration error: {0}")]
    ConfigError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Email service error (microservices mode)
    #[error("email service error: {0}")]
    ServiceError(String),
}

impl EmailError {
    /// Create an SMTP error from a string message
    #[must_use]
    pub fn smtp<T: Into<String>>(msg: T) -> Self {
        Self::SmtpError(msg.into())
    }

    /// Create an AWS SES error from a string message
    #[must_use]
    pub fn aws_ses<T: Into<String>>(msg: T) -> Self {
        Self::AwsSesError(msg.into())
    }

    /// Create a configuration error from a string message
    #[must_use]
    pub fn config<T: Into<String>>(msg: T) -> Self {
        Self::ConfigError(msg.into())
    }

    /// Create an email service error from a string message (microservices mode)
    #[must_use]
    pub fn service<T: Into<String>>(msg: T) -> Self {
        Self::ServiceError(msg.into())
    }
}

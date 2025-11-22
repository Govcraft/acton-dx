//! Email sending with multiple backends and template support
//!
//! This module provides a flexible email system with:
//! - Multiple backends (SMTP, AWS SES, console/development)
//! - Askama template integration for HTML and plain text emails
//! - Background job integration for async sending
//! - Common email flows (welcome, verification, password reset)
//!
//! # Examples
//!
//! ## Sending a simple email
//!
//! ```rust,no_run
//! use acton_htmx::email::{Email, SmtpBackend};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let backend = SmtpBackend::from_env()?;
//!
//! let email = Email::new()
//!     .to("user@example.com")
//!     .from("noreply@myapp.com")
//!     .subject("Welcome!")
//!     .text("Welcome to our app!")
//!     .html("<h1>Welcome to our app!</h1>");
//!
//! backend.send(email).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Using email templates
//!
//! ```rust,no_run
//! use acton_htmx::email::{Email, EmailTemplate};
//! use askama::Template;
//!
//! #[derive(Template)]
//! #[template(path = "emails/welcome.html")]
//! struct WelcomeEmail {
//!     name: String,
//!     verification_url: String,
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let template = WelcomeEmail {
//!     name: "Alice".to_string(),
//!     verification_url: "https://app.example.com/verify/abc123".to_string(),
//! };
//!
//! let email = Email::from_template(&template)?
//!     .to("alice@example.com")
//!     .from("noreply@myapp.com")
//!     .subject("Welcome to Our App!");
//!
//! # Ok(())
//! # }
//! ```

mod backend;
mod builder;
mod error;
mod job;
mod sender;
mod template;

#[cfg(test)]
mod testing;

pub use backend::{
    aws_ses::AwsSesBackend,
    console::ConsoleBackend,
    smtp::SmtpBackend,
};
pub use builder::Email;
pub use error::EmailError;
pub use job::SendEmailJob;
pub use sender::EmailSender;
pub use template::{EmailTemplate, SimpleEmailTemplate};

#[cfg(test)]
pub use testing::MockEmailSender;

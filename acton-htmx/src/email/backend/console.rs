//! Console backend for development
//!
//! Prints emails to the console instead of sending them.
//! Useful for development and testing.

use async_trait::async_trait;
use tracing::{debug, info};

use crate::email::{Email, EmailError, EmailSender};

/// Console email backend for development
///
/// Logs emails to the console instead of sending them.
/// Useful for development and testing without needing SMTP or AWS credentials.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::email::{Email, EmailSender, ConsoleBackend};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = ConsoleBackend::new();
///
/// let email = Email::new()
///     .to("user@example.com")
///     .from("noreply@myapp.com")
///     .subject("Hello!")
///     .text("Hello, World!");
///
/// backend.send(email).await?; // Prints to console
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct ConsoleBackend {
    /// Whether to log email content in debug mode
    verbose: bool,
}

impl ConsoleBackend {
    /// Create a new console backend
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a verbose console backend that logs full email content
    #[must_use]
    pub const fn verbose() -> Self {
        Self { verbose: true }
    }
}

#[async_trait]
impl EmailSender for ConsoleBackend {
    async fn send(&self, email: Email) -> Result<(), EmailError> {
        // Validate email first
        email.validate()?;

        let from = email.from.as_ref().ok_or(EmailError::NoSender)?;
        let subject = email.subject.as_ref().ok_or(EmailError::NoSubject)?;

        // Log email metadata
        info!(
            from = %from,
            to = ?email.to,
            cc = ?email.cc,
            bcc = ?email.bcc,
            subject = %subject,
            "Console email sent"
        );

        if self.verbose {
            debug!(
                reply_to = ?email.reply_to,
                has_html = email.html.is_some(),
                has_text = email.text.is_some(),
                headers = ?email.headers,
                "Email details"
            );

            if let Some(text) = &email.text {
                debug!(text = %text, "Email text content");
            }

            if let Some(html) = &email.html {
                debug!(html = %html, "Email HTML content");
            }
        }

        // Also print to stdout for visibility in development
        println!("\n╭─────────────────────────────────────────────────────╮");
        println!("│ 📧 Console Email                                     │");
        println!("├─────────────────────────────────────────────────────┤");
        println!("│ From:    {from:<43} │");
        println!("│ To:      {:<43} │", email.to.join(", "));
        if !email.cc.is_empty() {
            println!("│ CC:      {:<43} │", email.cc.join(", "));
        }
        if !email.bcc.is_empty() {
            println!("│ BCC:     {:<43} │", email.bcc.join(", "));
        }
        if let Some(reply_to) = &email.reply_to {
            println!("│ Reply-To: {reply_to:<42} │");
        }
        println!("│ Subject: {subject:<43} │");
        println!("├─────────────────────────────────────────────────────┤");

        if let Some(text) = &email.text {
            println!("│ Plain Text Content:                                 │");
            println!("├─────────────────────────────────────────────────────┤");
            for line in text.lines() {
                let truncated = if line.len() > 51 {
                    format!("{}...", &line[..48])
                } else {
                    line.to_string()
                };
                println!("│ {truncated:<51} │");
            }
            println!("├─────────────────────────────────────────────────────┤");
        }

        if let Some(html) = &email.html {
            println!("│ HTML Content:                                       │");
            println!("├─────────────────────────────────────────────────────┤");
            for line in html.lines().take(5) {
                let truncated = if line.len() > 51 {
                    format!("{}...", &line[..48])
                } else {
                    line.to_string()
                };
                println!("│ {truncated:<51} │");
            }
            if html.lines().count() > 5 {
                println!("│ ... (truncated)                                     │");
            }
            println!("├─────────────────────────────────────────────────────┤");
        }

        println!("╰─────────────────────────────────────────────────────╯\n");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_console_backend_send() {
        let backend = ConsoleBackend::new();

        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test Email")
            .text("This is a test email");

        let result = backend.send(email).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_console_backend_verbose() {
        let backend = ConsoleBackend::verbose();

        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test Email")
            .text("This is plain text")
            .html("<h1>This is HTML</h1>");

        let result = backend.send(email).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_console_backend_with_cc_and_bcc() {
        let backend = ConsoleBackend::new();

        let email = Email::new()
            .to("user@example.com")
            .cc("cc@example.com")
            .bcc("bcc@example.com")
            .from("noreply@myapp.com")
            .subject("Test Email")
            .text("Test content");

        let result = backend.send(email).await;
        assert!(result.is_ok());
    }
}

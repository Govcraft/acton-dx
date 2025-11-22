//! Email builder with fluent API
//!
//! Provides a convenient builder pattern for constructing emails.

use serde::{Deserialize, Serialize};

use super::{EmailError, EmailTemplate};

/// An email message
///
/// Use the builder pattern to construct emails:
///
/// ```rust
/// use acton_htmx::email::Email;
///
/// let email = Email::new()
///     .to("user@example.com")
///     .from("noreply@myapp.com")
///     .subject("Welcome!")
///     .text("Welcome to our app!")
///     .html("<h1>Welcome to our app!</h1>");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Email {
    /// Email recipients (To)
    pub to: Vec<String>,

    /// Email sender (From)
    pub from: Option<String>,

    /// Reply-To address
    pub reply_to: Option<String>,

    /// CC recipients
    pub cc: Vec<String>,

    /// BCC recipients
    pub bcc: Vec<String>,

    /// Email subject
    pub subject: Option<String>,

    /// Plain text body
    pub text: Option<String>,

    /// HTML body
    pub html: Option<String>,

    /// Custom headers
    pub headers: Vec<(String, String)>,
}

impl Email {
    /// Create a new empty email
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an email from a template
    ///
    /// # Errors
    ///
    /// Returns `EmailError::TemplateError` if the template fails to render
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use acton_htmx::email::{Email, EmailTemplate};
    /// use askama::Template;
    ///
    /// #[derive(Template)]
    /// #[template(path = "emails/welcome.html")]
    /// struct WelcomeEmail {
    ///     name: String,
    /// }
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let template = WelcomeEmail {
    ///     name: "Alice".to_string(),
    /// };
    ///
    /// let email = Email::from_template(&template)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_template<T: EmailTemplate>(template: &T) -> Result<Self, EmailError> {
        let (html, text) = template.render_email()?;

        let mut email = Self::new();
        if let Some(html_content) = html {
            email = email.html(&html_content);
        }
        if let Some(text_content) = text {
            email = email.text(&text_content);
        }

        Ok(email)
    }

    /// Add a recipient (To)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .to("user@example.com");
    /// ```
    #[must_use]
    pub fn to(mut self, address: &str) -> Self {
        self.to.push(address.to_string());
        self
    }

    /// Add multiple recipients (To)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .to_multiple(&["user1@example.com", "user2@example.com"]);
    /// ```
    #[must_use]
    pub fn to_multiple(mut self, addresses: &[&str]) -> Self {
        for address in addresses {
            self.to.push((*address).to_string());
        }
        self
    }

    /// Set the sender (From)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .from("noreply@myapp.com");
    /// ```
    #[must_use]
    pub fn from(mut self, address: &str) -> Self {
        self.from = Some(address.to_string());
        self
    }

    /// Set the reply-to address
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .reply_to("support@myapp.com");
    /// ```
    #[must_use]
    pub fn reply_to(mut self, address: &str) -> Self {
        self.reply_to = Some(address.to_string());
        self
    }

    /// Add a CC recipient
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .cc("manager@example.com");
    /// ```
    #[must_use]
    pub fn cc(mut self, address: &str) -> Self {
        self.cc.push(address.to_string());
        self
    }

    /// Add a BCC recipient
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .bcc("admin@example.com");
    /// ```
    #[must_use]
    pub fn bcc(mut self, address: &str) -> Self {
        self.bcc.push(address.to_string());
        self
    }

    /// Set the email subject
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .subject("Welcome to Our App!");
    /// ```
    #[must_use]
    pub fn subject(mut self, subject: &str) -> Self {
        self.subject = Some(subject.to_string());
        self
    }

    /// Set the plain text body
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .text("Welcome to our app!");
    /// ```
    #[must_use]
    pub fn text(mut self, body: &str) -> Self {
        self.text = Some(body.to_string());
        self
    }

    /// Set the HTML body
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .html("<h1>Welcome to our app!</h1>");
    /// ```
    #[must_use]
    pub fn html(mut self, body: &str) -> Self {
        self.html = Some(body.to_string());
        self
    }

    /// Add a custom header
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::email::Email;
    ///
    /// let email = Email::new()
    ///     .header("X-Priority", "1");
    /// ```
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Validate the email
    ///
    /// Checks that all required fields are present
    ///
    /// # Errors
    ///
    /// Returns errors if:
    /// - No recipients
    /// - No sender
    /// - No subject
    /// - No content (text or HTML)
    pub fn validate(&self) -> Result<(), EmailError> {
        if self.to.is_empty() && self.cc.is_empty() && self.bcc.is_empty() {
            return Err(EmailError::NoRecipients);
        }

        if self.from.is_none() {
            return Err(EmailError::NoSender);
        }

        if self.subject.is_none() {
            return Err(EmailError::NoSubject);
        }

        if self.text.is_none() && self.html.is_none() {
            return Err(EmailError::NoContent);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_builder() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello, World!");

        assert_eq!(email.to, vec!["user@example.com"]);
        assert_eq!(email.from, Some("noreply@myapp.com".to_string()));
        assert_eq!(email.subject, Some("Test".to_string()));
        assert_eq!(email.text, Some("Hello, World!".to_string()));
    }

    #[test]
    fn test_email_validation_no_recipients() {
        let email = Email::new()
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        assert!(matches!(email.validate(), Err(EmailError::NoRecipients)));
    }

    #[test]
    fn test_email_validation_no_sender() {
        let email = Email::new()
            .to("user@example.com")
            .subject("Test")
            .text("Hello");

        assert!(matches!(email.validate(), Err(EmailError::NoSender)));
    }

    #[test]
    fn test_email_validation_no_subject() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .text("Hello");

        assert!(matches!(email.validate(), Err(EmailError::NoSubject)));
    }

    #[test]
    fn test_email_validation_no_content() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test");

        assert!(matches!(email.validate(), Err(EmailError::NoContent)));
    }

    #[test]
    fn test_email_validation_success() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello, World!");

        assert!(email.validate().is_ok());
    }

    #[test]
    fn test_multiple_recipients() {
        let email = Email::new()
            .to_multiple(&["user1@example.com", "user2@example.com"])
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        assert_eq!(email.to.len(), 2);
        assert!(email.to.contains(&"user1@example.com".to_string()));
        assert!(email.to.contains(&"user2@example.com".to_string()));
    }

    #[test]
    fn test_cc_and_bcc() {
        let email = Email::new()
            .to("user@example.com")
            .cc("manager@example.com")
            .bcc("admin@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello");

        assert_eq!(email.cc, vec!["manager@example.com"]);
        assert_eq!(email.bcc, vec!["admin@example.com"]);
    }

    #[test]
    fn test_custom_headers() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Hello")
            .header("X-Priority", "1")
            .header("X-Custom", "value");

        assert_eq!(email.headers.len(), 2);
        assert!(email.headers.contains(&("X-Priority".to_string(), "1".to_string())));
        assert!(email.headers.contains(&("X-Custom".to_string(), "value".to_string())));
    }

    #[test]
    fn test_html_and_text() {
        let email = Email::new()
            .to("user@example.com")
            .from("noreply@myapp.com")
            .subject("Test")
            .text("Plain text content")
            .html("<h1>HTML content</h1>");

        assert_eq!(email.text, Some("Plain text content".to_string()));
        assert_eq!(email.html, Some("<h1>HTML content</h1>".to_string()));
    }
}

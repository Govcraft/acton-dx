//! Email template trait for Askama integration
//!
//! Provides a trait for rendering email templates with both HTML and plain text versions.

use super::EmailError;

/// Trait for email templates
///
/// Implement this trait on your Askama templates to render emails with both
/// HTML and plain text versions.
///
/// # Examples
///
/// ```rust,no_run
/// use acton_htmx::email::{EmailTemplate, EmailError};
/// use askama::Template;
///
/// #[derive(Template)]
/// #[template(path = "emails/welcome.html")]
/// struct WelcomeEmail {
///     name: String,
///     verification_url: String,
/// }
///
/// #[derive(Template)]
/// #[template(path = "emails/welcome.txt")]
/// struct WelcomeEmailText {
///     name: String,
///     verification_url: String,
/// }
///
/// impl EmailTemplate for WelcomeEmail {
///     fn render_email(&self) -> Result<(Option<String>, Option<String>), EmailError> {
///         let html = self.render()?;
///         let text_template = WelcomeEmailText {
///             name: self.name.clone(),
///             verification_url: self.verification_url.clone(),
///         };
///         let text = text_template.render()?;
///         Ok((Some(html), Some(text)))
///     }
/// }
/// ```
pub trait EmailTemplate {
    /// Render the email template
    ///
    /// Returns a tuple of `(html, text)` where either can be `None`.
    /// Most emails should provide both HTML and plain text versions.
    ///
    /// # Errors
    ///
    /// Returns `EmailError::TemplateError` if the template fails to render
    fn render_email(&self) -> Result<(Option<String>, Option<String>), EmailError>;
}

/// Helper trait for rendering both HTML and text versions from a single template
///
/// This trait provides a default implementation that uses the same template
/// for both HTML and text. For production use, you should create separate
/// templates for HTML and text versions.
pub trait SimpleEmailTemplate: askama::Template {
    /// Render the template as HTML
    ///
    /// # Errors
    ///
    /// Returns `EmailError::TemplateError` if the template fails to render
    fn render_html(&self) -> Result<String, EmailError> {
        Ok(self.render()?)
    }

    /// Render a plain text version
    ///
    /// Default implementation returns `None`. Override this to provide
    /// a plain text version.
    ///
    /// # Errors
    ///
    /// Returns `EmailError::TemplateError` if the template fails to render
    fn render_text(&self) -> Result<Option<String>, EmailError> {
        Ok(None)
    }
}

impl<T: SimpleEmailTemplate> EmailTemplate for T {
    fn render_email(&self) -> Result<(Option<String>, Option<String>), EmailError> {
        let html = Some(self.render_html()?);
        let text = self.render_text()?;
        Ok((html, text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use askama::Template;

    #[derive(Template)]
    #[template(source = "<h1>Hello, {{ name }}!</h1>", ext = "html")]
    struct TestTemplate {
        name: String,
    }

    impl SimpleEmailTemplate for TestTemplate {}

    #[test]
    fn test_simple_email_template() {
        let template = TestTemplate {
            name: "Alice".to_string(),
        };

        let (html, text) = template.render_email().unwrap();

        assert!(html.is_some());
        assert_eq!(html.unwrap(), "<h1>Hello, Alice!</h1>");
        assert!(text.is_none());
    }

    #[derive(Template)]
    #[template(source = "<h1>Welcome, {{ name }}!</h1>", ext = "html")]
    struct TestTemplateWithText {
        name: String,
    }

    impl SimpleEmailTemplate for TestTemplateWithText {
        fn render_text(&self) -> Result<Option<String>, EmailError> {
            Ok(Some(format!("Welcome, {}!", self.name)))
        }
    }

    #[test]
    fn test_email_template_with_text() {
        let template = TestTemplateWithText {
            name: "Bob".to_string(),
        };

        let (html, text) = template.render_email().unwrap();

        assert!(html.is_some());
        assert_eq!(html.unwrap(), "<h1>Welcome, Bob!</h1>");
        assert!(text.is_some());
        assert_eq!(text.unwrap(), "Welcome, Bob!");
    }
}

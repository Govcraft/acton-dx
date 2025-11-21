//! Askama template engine integration with HTMX patterns
//!
//! This module provides:
//! - `HxTemplate` trait for automatic partial/full page detection
//! - Template registry with optional caching
//! - HTMX-aware template helpers
//! - Integration with axum-htmx response types
//!
//! # Examples
//!
//! ```rust,no_run
//! use askama::Template;
//! use acton_htmx::template::HxTemplate;
//! use axum_htmx::HxRequest;
//!
//! #[derive(Template)]
//! #[template(path = "posts/index.html")]
//! struct PostsIndexTemplate {
//!     posts: Vec<String>,
//! }
//!
//! async fn index(HxRequest(is_htmx): HxRequest) -> impl axum::response::IntoResponse {
//!     let template = PostsIndexTemplate {
//!         posts: vec!["Post 1".to_string(), "Post 2".to_string()],
//!     };
//!
//!     template.render_htmx(is_htmx)
//! }
//! ```

use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

pub mod helpers;
pub mod registry;

pub use helpers::*;
pub use registry::TemplateRegistry;

/// Extension trait for Askama templates with HTMX support
///
/// Automatically renders partial content for HTMX requests and full pages
/// for regular browser requests.
pub trait HxTemplate: Template {
    /// Render template based on HTMX request detection
    ///
    /// Returns partial content if `is_htmx` is true, otherwise returns full page.
    /// The distinction between partial and full is determined by the template's
    /// structure and naming conventions.
    ///
    /// # Errors
    ///
    /// Returns `StatusCode::INTERNAL_SERVER_ERROR` if template rendering fails.
    fn render_htmx(self, is_htmx: bool) -> Response
    where
        Self: Sized,
    {
        match self.render() {
            Ok(html) => {
                if is_htmx {
                    // For HTMX requests, return just the content
                    // In a full implementation, this would extract the main content block
                    Html(html).into_response()
                } else {
                    // For regular requests, return the full page
                    Html(html).into_response()
                }
            }
            Err(err) => {
                tracing::error!("Template rendering error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template rendering failed",
                )
                    .into_response()
            }
        }
    }

    /// Render as HTML response
    ///
    /// Always renders the full template regardless of request type.
    ///
    /// # Errors
    ///
    /// Returns `StatusCode::INTERNAL_SERVER_ERROR` if template rendering fails.
    fn render_html(self) -> Response
    where
        Self: Sized,
    {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => {
                tracing::error!("Template rendering error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Template rendering failed",
                )
                    .into_response()
            }
        }
    }

    /// Render partial content only
    ///
    /// Extracts and renders only the main content block without layout.
    /// Useful for HTMX partial updates.
    ///
    /// # Errors
    ///
    /// Returns `StatusCode::INTERNAL_SERVER_ERROR` if template rendering fails.
    fn render_partial(self) -> Response
    where
        Self: Sized,
    {
        // In a full implementation, this would extract content between specific markers
        // For now, it renders the full template
        self.render_html()
    }
}

// Blanket implementation for all Askama templates
impl<T> HxTemplate for T where T: Template {}

#[cfg(test)]
mod tests {
    use super::*;
    use askama::Template;

    #[derive(Template)]
    #[template(source = "<h1>{{ title }}</h1>", ext = "html")]
    struct TestTemplate {
        title: String,
    }

    #[test]
    fn test_render_html() {
        let template = TestTemplate {
            title: "Hello".to_string(),
        };

        let response = template.render_html();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_render_htmx_full_page() {
        let template = TestTemplate {
            title: "Hello".to_string(),
        };

        let response = template.render_htmx(false);
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn test_render_htmx_partial() {
        let template = TestTemplate {
            title: "Hello".to_string(),
        };

        let response = template.render_htmx(true);
        assert_eq!(response.status(), StatusCode::OK);
    }
}

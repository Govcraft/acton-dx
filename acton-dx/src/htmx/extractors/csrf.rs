//! CSRF token extractors for Axum handlers
//!
//! Provides extractors that allow handlers to access CSRF tokens for rendering in templates.

use crate::htmx::agents::{CsrfToken, GetOrCreateToken};
use crate::htmx::auth::session::SessionId;
use crate::htmx::state::ActonHtmxState;
use acton_reactive::prelude::ActorHandleInterface;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use std::time::Duration;

/// Extractor for CSRF token
///
/// Retrieves or creates a CSRF token for the current session.
/// Requires SessionMiddleware to be applied first.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::extractors::CsrfTokenExtractor;
/// use axum::{response::Html, extract::State};
///
/// async fn render_form(csrf: CsrfTokenExtractor) -> Html<String> {
///     let token = csrf.token();
///     Html(format!(
///         r#"<form method="post">
///             <input type="hidden" name="_csrf_token" value="{token}">
///             <button type="submit">Submit</button>
///         </form>"#
///     ))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CsrfTokenExtractor {
    token: CsrfToken,
}

impl CsrfTokenExtractor {
    /// Get the CSRF token as a string
    #[must_use]
    pub fn token(&self) -> &str {
        self.token.as_str()
    }

    /// Get the CSRF token value
    #[must_use]
    pub const fn value(&self) -> &CsrfToken {
        &self.token
    }
}

impl<S> FromRequestParts<S> for CsrfTokenExtractor
where
    S: Send + Sync,
    ActonHtmxState: axum::extract::FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract state
        let state = ActonHtmxState::from_ref(state);

        // Get session ID from extensions (set by SessionMiddleware)
        let session_id = parts
            .extensions
            .get::<SessionId>()
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Session not found - ensure SessionMiddleware is applied".to_string(),
                )
            })?;

        // Get or create CSRF token from CSRF manager
        let (request, rx) = GetOrCreateToken::new(session_id);
        state.csrf_manager().send(request).await;

        // Wait for response with timeout
        let timeout = Duration::from_millis(100);
        let token = tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CSRF token retrieval timeout".to_string(),
                )
            })?
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CSRF token retrieval error".to_string(),
                )
            })?;

        Ok(Self { token })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csrf_token_extractor_creation() {
        let token = CsrfToken::generate();
        let extractor = CsrfTokenExtractor {
            token: token.clone(),
        };

        assert_eq!(extractor.token(), token.as_str());
        assert_eq!(extractor.value(), &token);
    }

    #[test]
    fn test_csrf_token_extractor_debug() {
        let token = CsrfToken::generate();
        let extractor = CsrfTokenExtractor { token };

        let debug_str = format!("{extractor:?}");
        assert!(debug_str.contains("CsrfTokenExtractor"));
    }

    #[test]
    fn test_csrf_token_extractor_clone() {
        let token = CsrfToken::generate();
        let extractor = CsrfTokenExtractor { token };
        let cloned = extractor.clone();

        assert_eq!(extractor.token(), cloned.token());
    }
}

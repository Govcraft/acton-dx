//! Session and flash message extractors
//!
//! Provides axum extractors for accessing session data and flash messages
//! within request handlers.

use crate::auth::session::{FlashMessage, SessionData, SessionId};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use std::convert::Infallible;

/// Extractor for session data
///
/// Extracts the current session from request extensions.
/// Requires `SessionMiddleware` to be applied to the router.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::extractors::SessionExtractor;
///
/// async fn handler(SessionExtractor(session_id, session): SessionExtractor) {
///     if let Some(user_id) = session.user_id {
///         // User is authenticated
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SessionExtractor(pub SessionId, pub SessionData);

impl<S> FromRequestParts<S> for SessionExtractor
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let session_id = parts
            .extensions
            .get::<SessionId>()
            .cloned()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Session not initialized"))?;

        let session_data = parts
            .extensions
            .get::<SessionData>()
            .cloned()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Session data not found"))?;

        Ok(Self(session_id, session_data))
    }
}

/// Extractor for flash messages
///
/// Extracts flash messages from the session, consuming them.
/// Messages are typically shown once and then cleared.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::extractors::FlashExtractor;
///
/// async fn handler(FlashExtractor(messages): FlashExtractor) {
///     for msg in messages {
///         println!("Flash: {} - {}", msg.level, msg.content);
///     }
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct FlashExtractor(pub Vec<FlashMessage>);

impl<S> FromRequestParts<S> for FlashExtractor
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get flash messages from session data in extensions
        let messages = parts
            .extensions
            .get::<SessionData>()
            .map(|session| session.flash_messages.clone())
            .unwrap_or_default();

        // TODO: Clear flash messages from session after extraction
        // This requires mutable access or a separate mechanism

        Ok(Self(messages))
    }
}

/// Optional session extractor
///
/// Returns `None` if session is not available, rather than failing.
/// Useful for routes that can work with or without a session.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::extractors::OptionalSession;
///
/// async fn handler(OptionalSession(session): OptionalSession) {
///     match session {
///         Some((id, data)) => { /* Authenticated */ }
///         None => { /* Anonymous */ }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct OptionalSession(pub Option<(SessionId, SessionData)>);

impl<S> FromRequestParts<S> for OptionalSession
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let session = parts
            .extensions
            .get::<SessionId>()
            .cloned()
            .and_then(|id| {
                parts
                    .extensions
                    .get::<SessionData>()
                    .cloned()
                    .map(|data| (id, data))
            });

        Ok(Self(session))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_extractor_default() {
        let flash = FlashExtractor::default();
        assert!(flash.0.is_empty());
    }

    #[test]
    fn test_optional_session_default() {
        // Just verify the types compile correctly
        let _session: OptionalSession = OptionalSession(None);
    }
}

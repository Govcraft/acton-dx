//! Authentication extractors for Axum handlers
//!
//! Provides extractors for accessing authenticated users in request handlers.
//!
//! # Examples
//!
//! ## Requiring authentication
//!
//! ```rust,no_run
//! use acton_htmx::auth::{Authenticated, User};
//! use axum::response::IntoResponse;
//!
//! async fn protected_handler(
//!     Authenticated(user): Authenticated<User>,
//! ) -> impl IntoResponse {
//!     format!("Hello, {}!", user.email)
//! }
//! ```
//!
//! ## Optional authentication
//!
//! ```rust,no_run
//! use acton_htmx::auth::{OptionalAuth, User};
//! use axum::response::IntoResponse;
//!
//! async fn optional_handler(
//!     OptionalAuth(user): OptionalAuth<User>,
//! ) -> impl IntoResponse {
//!     match user {
//!         Some(user) => format!("Hello, {}!", user.email),
//!         None => "Hello, guest!".to_string(),
//!     }
//! }
//! ```

use crate::auth::{Session, User, UserError};
use crate::middleware::is_htmx_request;
use crate::state::ActonHtmxState;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect, Response},
};

/// Authenticated user extractor for protected routes
///
/// This extractor ensures that a user is authenticated before the handler runs.
/// If no valid session exists, it returns an appropriate error response:
/// - For HTMX requests: 401 Unauthorized with HX-Redirect header
/// - For regular requests: 303 redirect to `/login`
///
/// # Example
///
/// ```rust,no_run
/// use acton_htmx::auth::{Authenticated, User};
///
/// async fn protected_handler(
///     Authenticated(user): Authenticated<User>,
/// ) -> String {
///     format!("User ID: {}", user.id)
/// }
/// ```
pub struct Authenticated<T>(pub T);

impl<S> FromRequestParts<S> for Authenticated<User>
where
    S: Send + Sync,
    ActonHtmxState: FromRef<S>,
{
    type Rejection = AuthenticationError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Check if this is an HTMX request
        let is_htmx = is_htmx_request(&parts.headers);

        // Get session from request extensions
        let session = parts
            .extensions
            .get::<Session>()
            .cloned()
            .ok_or_else(|| AuthenticationError::missing_session(is_htmx))?;

        // Check if user is authenticated
        let user_id = session
            .user_id()
            .ok_or_else(|| AuthenticationError::not_authenticated(is_htmx))?;

        // Extract state to get database pool
        let app_state = ActonHtmxState::from_ref(state);

        // Load user from database
        let user = User::find_by_id(user_id, app_state.database_pool())
            .await
            .map_err(|e| match e {
                UserError::NotFound => AuthenticationError::not_authenticated(is_htmx),
                _ => AuthenticationError::DatabaseError(e),
            })?;

        Ok(Self(user))
    }
}

/// Optional authentication extractor
///
/// This extractor works for both authenticated and unauthenticated requests.
/// It returns `Some(user)` if authenticated, `None` otherwise.
///
/// # Example
///
/// ```rust,no_run
/// use acton_htmx::auth::{OptionalAuth, User};
///
/// async fn optional_handler(
///     OptionalAuth(user): OptionalAuth<User>,
/// ) -> String {
///     match user {
///         Some(u) => format!("Hello, {}!", u.email),
///         None => "Hello, guest!".to_string(),
///     }
/// }
/// ```
pub struct OptionalAuth<T>(pub Option<T>);

impl<S> FromRequestParts<S> for OptionalAuth<User>
where
    S: Send + Sync,
    ActonHtmxState: FromRef<S>,
{
    type Rejection = AuthenticationError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Get session from request extensions
        let Some(session) = parts.extensions.get::<Session>().cloned() else {
            return Ok(Self(None)); // No session = not authenticated
        };

        // Check if user is authenticated
        let Some(user_id) = session.user_id() else {
            return Ok(Self(None)); // No user_id = not authenticated
        };

        // Extract state to get database pool
        let app_state = ActonHtmxState::from_ref(state);

        // Load user from database
        let user = User::find_by_id(user_id, app_state.database_pool())
            .await
            .ok(); // Convert Result to Option - failures return None

        Ok(Self(user))
    }
}

/// Authentication errors for extractors
#[derive(Debug)]
pub enum AuthenticationError {
    /// No session found in request extensions (HTMX request)
    MissingSessionHtmx,

    /// No session found in request extensions (regular request)
    MissingSession,

    /// Session exists but user is not authenticated (HTMX request)
    NotAuthenticatedHtmx,

    /// Session exists but user is not authenticated (regular request)
    NotAuthenticated,

    /// Database not configured (development/testing)
    DatabaseNotConfigured,

    /// Database error occurred
    DatabaseError(UserError),
}

impl AuthenticationError {
    /// Create a "missing session" error appropriate for the request type.
    ///
    /// This helper reduces duplication by encapsulating the HTMX detection logic.
    ///
    /// # Arguments
    ///
    /// * `is_htmx` - Whether the request is from HTMX
    ///
    /// # Returns
    ///
    /// * [`MissingSessionHtmx`](Self::MissingSessionHtmx) for HTMX requests
    /// * [`MissingSession`](Self::MissingSession) for regular requests
    #[must_use]
    pub const fn missing_session(is_htmx: bool) -> Self {
        if is_htmx {
            Self::MissingSessionHtmx
        } else {
            Self::MissingSession
        }
    }

    /// Create a "not authenticated" error appropriate for the request type.
    ///
    /// This helper reduces duplication by encapsulating the HTMX detection logic.
    ///
    /// # Arguments
    ///
    /// * `is_htmx` - Whether the request is from HTMX
    ///
    /// # Returns
    ///
    /// * [`NotAuthenticatedHtmx`](Self::NotAuthenticatedHtmx) for HTMX requests
    /// * [`NotAuthenticated`](Self::NotAuthenticated) for regular requests
    #[must_use]
    pub const fn not_authenticated(is_htmx: bool) -> Self {
        if is_htmx {
            Self::NotAuthenticatedHtmx
        } else {
            Self::NotAuthenticated
        }
    }
}

impl IntoResponse for AuthenticationError {
    fn into_response(self) -> Response {
        match self {
            Self::MissingSessionHtmx | Self::NotAuthenticatedHtmx => {
                // For HTMX requests, return 401 with HX-Redirect header
                (
                    StatusCode::UNAUTHORIZED,
                    [("HX-Redirect", "/login")],
                    "Unauthorized",
                )
                    .into_response()
            }
            Self::MissingSession | Self::NotAuthenticated => {
                // For regular requests, redirect to login
                Redirect::to("/login").into_response()
            }
            Self::DatabaseNotConfigured => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database not configured",
                )
                    .into_response()
            }
            Self::DatabaseError(_) => {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to load user",
                )
                    .into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_authentication_error_missing_session_regular_returns_redirect() {
        let error = AuthenticationError::MissingSession;
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap(),
            "/login"
        );
    }

    #[test]
    fn test_authentication_error_missing_session_htmx_returns_401_with_hx_redirect() {
        let error = AuthenticationError::MissingSessionHtmx;
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("HX-Redirect").unwrap(),
            "/login"
        );
    }

    #[test]
    fn test_authentication_error_not_authenticated_regular_returns_redirect() {
        let error = AuthenticationError::NotAuthenticated;
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap(),
            "/login"
        );
    }

    #[test]
    fn test_authentication_error_not_authenticated_htmx_returns_401_with_hx_redirect() {
        let error = AuthenticationError::NotAuthenticatedHtmx;
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("HX-Redirect").unwrap(),
            "/login"
        );
    }

    #[test]
    fn test_authentication_error_database_not_configured_returns_500() {
        let error = AuthenticationError::DatabaseNotConfigured;
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_authentication_error_database_error_returns_500() {
        let error = AuthenticationError::DatabaseError(UserError::NotFound);
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_missing_session_helper_returns_htmx_variant_when_is_htmx_true() {
        let error = AuthenticationError::missing_session(true);
        assert!(matches!(error, AuthenticationError::MissingSessionHtmx));
    }

    #[test]
    fn test_missing_session_helper_returns_regular_variant_when_is_htmx_false() {
        let error = AuthenticationError::missing_session(false);
        assert!(matches!(error, AuthenticationError::MissingSession));
    }

    #[test]
    fn test_not_authenticated_helper_returns_htmx_variant_when_is_htmx_true() {
        let error = AuthenticationError::not_authenticated(true);
        assert!(matches!(error, AuthenticationError::NotAuthenticatedHtmx));
    }

    #[test]
    fn test_not_authenticated_helper_returns_regular_variant_when_is_htmx_false() {
        let error = AuthenticationError::not_authenticated(false);
        assert!(matches!(error, AuthenticationError::NotAuthenticated));
    }
}

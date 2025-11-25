//! Authentication middleware for protecting routes
//!
//! This module provides middleware for requiring authentication on routes
//! and extractors for accessing the authenticated user.
//!
//! # Example
//!
//! ```rust,no_run
//! use acton_htmx::middleware::AuthMiddleware;
//! use acton_htmx::auth::Authenticated;
//! use axum::{Router, routing::get, middleware};
//!
//! async fn protected_handler(
//!     Authenticated(user): Authenticated<acton_htmx::auth::User>,
//! ) -> String {
//!     format!("Hello, {}!", user.email)
//! }
//!
//! # async fn example() {
//! // Default login path (/login)
//! let app = Router::new()
//!     .route("/protected", get(protected_handler))
//!     .layer(middleware::from_fn(AuthMiddleware::handle));
//!
//! // Custom login path
//! let custom_middleware = AuthMiddleware::with_login_path("/auth/login");
//! let app = Router::new()
//!     .route("/protected", get(protected_handler))
//!     .layer(middleware::from_fn(move |req, next| {
//!         custom_middleware.clone().handle_with_config(req, next)
//!     }));
//! # }
//! ```

use super::helpers::is_htmx_request;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};

/// Middleware that requires authentication for routes
///
/// If the user is not authenticated, they will be redirected to the login page.
/// For HTMX requests, returns a 401 Unauthorized status with HX-Redirect header.
///
/// # Login Path Configuration
///
/// By default, unauthenticated users are redirected to `/login`. This can be
/// customized using [`AuthMiddleware::with_login_path`].
#[derive(Clone, Debug)]
pub struct AuthMiddleware {
    login_path: String,
}

impl Default for AuthMiddleware {
    fn default() -> Self {
        Self {
            login_path: "/login".to_string(),
        }
    }
}

impl AuthMiddleware {
    /// Create a new authentication middleware with default settings
    ///
    /// By default, redirects to `/login` for unauthenticated requests.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create authentication middleware with custom login path
    ///
    /// # Example
    ///
    /// ```rust
    /// use acton_htmx::middleware::AuthMiddleware;
    ///
    /// let middleware = AuthMiddleware::with_login_path("/auth/login");
    /// ```
    #[must_use]
    pub fn with_login_path(login_path: impl Into<String>) -> Self {
        Self {
            login_path: login_path.into(),
        }
    }

    /// Middleware handler that checks for authentication with default login path
    ///
    /// This is a convenience method that uses the default login path `/login`.
    /// For custom login paths, use [`AuthMiddleware::with_login_path`] and
    /// [`AuthMiddleware::handle_with_config`].
    ///
    /// # Errors
    ///
    /// Returns [`AuthMiddlewareError`] if:
    /// - No valid session exists in request extensions
    /// - Session exists but does not contain a user_id
    ///
    /// For HTMX requests, returns 401 with HX-Redirect header to login page.
    /// For standard browser requests, redirects to login page.
    pub async fn handle(
        request: Request,
        next: Next,
    ) -> Result<Response, AuthMiddlewareError> {
        Self::default().handle_with_config(request, next).await
    }

    /// Middleware handler that checks for authentication with configured login path
    ///
    /// This method uses the login path configured in this middleware instance.
    ///
    /// # Errors
    ///
    /// Returns [`AuthMiddlewareError`] if:
    /// - No valid session exists in request extensions
    /// - Session exists but does not contain a user_id
    ///
    /// For HTMX requests, returns 401 with HX-Redirect header to configured login page.
    /// For standard browser requests, redirects to configured login page.
    pub async fn handle_with_config(
        self,
        request: Request,
        next: Next,
    ) -> Result<Response, AuthMiddlewareError> {
        // Check if user is authenticated by looking for user_id in session
        let (parts, body) = request.into_parts();

        // Get session from request extensions
        let session = parts.extensions.get::<crate::auth::Session>().cloned();

        let is_authenticated = session
            .as_ref()
            .and_then(super::super::auth::Session::user_id)
            .is_some();

        if !is_authenticated {
            // Use helper to create appropriate error for request type
            return Err(AuthMiddlewareError::for_request(
                is_htmx_request(&parts.headers),
                self.login_path,
            ));
        }

        // User is authenticated, continue with the request
        let request = Request::from_parts(parts, body);
        Ok(next.run(request).await)
    }
}

/// Authentication middleware errors
#[derive(Debug)]
pub enum AuthMiddlewareError {
    /// User is not authenticated (HTMX request)
    ///
    /// Contains the login path to redirect to
    Unauthorized(String),
    /// Redirect to login page (regular request)
    ///
    /// Contains the login path to redirect to
    RedirectToLogin(String),
}

impl AuthMiddlewareError {
    /// Create an authentication error appropriate for the request type.
    ///
    /// This helper reduces duplication by encapsulating the HTMX detection logic.
    ///
    /// # Arguments
    ///
    /// * `is_htmx` - Whether the request is from HTMX
    /// * `login_path` - The path to redirect to for login
    ///
    /// # Returns
    ///
    /// * [`Unauthorized`](Self::Unauthorized) for HTMX requests (returns 401 with HX-Redirect)
    /// * [`RedirectToLogin`](Self::RedirectToLogin) for regular requests (returns 303 redirect)
    #[must_use]
    pub fn for_request(is_htmx: bool, login_path: impl Into<String>) -> Self {
        let login_path = login_path.into();
        if is_htmx {
            Self::Unauthorized(login_path)
        } else {
            Self::RedirectToLogin(login_path)
        }
    }
}

impl IntoResponse for AuthMiddlewareError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized(login_path) => {
                // Return 401 with HX-Redirect header for HTMX
                (
                    StatusCode::UNAUTHORIZED,
                    [("HX-Redirect", login_path.as_str())],
                    "Unauthorized",
                )
                    .into_response()
            }
            Self::RedirectToLogin(login_path) => {
                // Regular HTTP redirect
                Redirect::to(&login_path).into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{Session, SessionData, SessionId};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn protected_handler() -> &'static str {
        "Protected content"
    }

    #[tokio::test]
    async fn test_unauthenticated_regular_request_redirects() {
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(AuthMiddleware::handle));

        let request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should redirect to login
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap(),
            "/login"
        );
    }

    #[tokio::test]
    async fn test_unauthenticated_htmx_request_returns_401() {
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(AuthMiddleware::handle));

        let request = Request::builder()
            .uri("/protected")
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return 401 with HX-Redirect header
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("HX-Redirect").unwrap(),
            "/login"
        );
    }

    #[tokio::test]
    async fn test_authenticated_request_proceeds() {
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(AuthMiddleware::handle));

        let mut request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        // Add authenticated session to request extensions
        let session_id = SessionId::generate();
        let mut session_data = SessionData::new();
        session_data.user_id = Some(1);
        let session = Session::new(session_id, session_data);

        request.extensions_mut().insert(session);

        let response = app.oneshot(request).await.unwrap();

        // Should proceed to handler
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_custom_login_path_regular_request() {
        let custom_middleware = AuthMiddleware::with_login_path("/auth/signin");
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |req, next| {
                custom_middleware.clone().handle_with_config(req, next)
            }));

        let request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should redirect to custom login path
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap(),
            "/auth/signin"
        );
    }

    #[tokio::test]
    async fn test_custom_login_path_htmx_request() {
        let custom_middleware = AuthMiddleware::with_login_path("/auth/signin");
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |req, next| {
                custom_middleware.clone().handle_with_config(req, next)
            }));

        let request = Request::builder()
            .uri("/protected")
            .header("HX-Request", "true")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return 401 with HX-Redirect to custom login path
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("HX-Redirect").unwrap(),
            "/auth/signin"
        );
    }

    #[tokio::test]
    async fn test_custom_login_path_with_authenticated_request() {
        let custom_middleware = AuthMiddleware::with_login_path("/auth/signin");
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn(move |req, next| {
                custom_middleware.clone().handle_with_config(req, next)
            }));

        let mut request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        // Add authenticated session to request extensions
        let session_id = SessionId::generate();
        let mut session_data = SessionData::new();
        session_data.user_id = Some(1);
        let session = Session::new(session_id, session_data);

        request.extensions_mut().insert(session);

        let response = app.oneshot(request).await.unwrap();

        // Should proceed to handler regardless of custom login path
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_default_login_path_is_slash_login() {
        let middleware = AuthMiddleware::new();
        assert_eq!(middleware.login_path, "/login");

        let default_middleware = AuthMiddleware::default();
        assert_eq!(default_middleware.login_path, "/login");
    }

    #[tokio::test]
    async fn test_with_login_path_accepts_string() {
        let middleware = AuthMiddleware::with_login_path("/custom".to_string());
        assert_eq!(middleware.login_path, "/custom");
    }

    #[tokio::test]
    async fn test_with_login_path_accepts_str() {
        let middleware = AuthMiddleware::with_login_path("/custom");
        assert_eq!(middleware.login_path, "/custom");
    }

    #[test]
    fn test_for_request_returns_unauthorized_when_htmx() {
        let error = AuthMiddlewareError::for_request(true, "/login");
        assert!(matches!(error, AuthMiddlewareError::Unauthorized(path) if path == "/login"));
    }

    #[test]
    fn test_for_request_returns_redirect_when_not_htmx() {
        let error = AuthMiddlewareError::for_request(false, "/login");
        assert!(matches!(error, AuthMiddlewareError::RedirectToLogin(path) if path == "/login"));
    }

    #[test]
    fn test_for_request_accepts_string() {
        let error = AuthMiddlewareError::for_request(true, "/custom/login".to_string());
        assert!(matches!(error, AuthMiddlewareError::Unauthorized(path) if path == "/custom/login"));
    }
}

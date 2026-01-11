//! CSRF middleware for protection against Cross-Site Request Forgery attacks
//!
//! Provides middleware that validates CSRF tokens on state-changing requests
//! (POST, PUT, DELETE, PATCH). Integrates with the `CsrfManagerAgent` for
//! token storage and validation.
//!
//! # Security Features
//!
//! - Automatic token validation on non-idempotent methods
//! - Token rotation after successful validation
//! - 403 Forbidden response on validation failure
//! - Support for both form data and custom headers
//! - Session-based token storage

use crate::htmx::agents::{CsrfToken, ValidateToken};
use crate::htmx::auth::session::SessionId;
use crate::htmx::state::ActonHtmxState;
use acton_reactive::prelude::{ActorHandle, ActorHandleInterface};
use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Layer, Service};

/// CSRF token header name
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

/// CSRF token form field name
pub const CSRF_FORM_FIELD: &str = "_csrf_token";

/// CSRF configuration for middleware
#[derive(Clone, Debug)]
pub struct CsrfConfig {
    /// Header name for CSRF token (default: "x-csrf-token")
    pub header_name: String,
    /// Form field name for CSRF token (default: "_csrf_token")
    pub form_field: String,
    /// Timeout for agent communication in milliseconds
    pub agent_timeout_ms: u64,
    /// Skip CSRF validation for these paths (e.g., webhooks, health checks)
    pub skip_paths: Vec<String>,
}

impl Default for CsrfConfig {
    fn default() -> Self {
        Self {
            header_name: CSRF_HEADER_NAME.to_string(),
            form_field: CSRF_FORM_FIELD.to_string(),
            agent_timeout_ms: 100,
            skip_paths: vec![],
        }
    }
}

impl CsrfConfig {
    /// Create new CSRF config with default values
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a path to skip CSRF validation
    #[must_use]
    pub fn skip_path(mut self, path: impl Into<String>) -> Self {
        self.skip_paths.push(path.into());
        self
    }

    /// Add multiple paths to skip CSRF validation
    #[must_use]
    pub fn skip_paths(mut self, paths: Vec<String>) -> Self {
        self.skip_paths.extend(paths);
        self
    }
}

/// Layer for CSRF middleware
///
/// Requires both `SessionId` and CSRF manager to be available.
#[derive(Clone)]
pub struct CsrfLayer {
    config: CsrfConfig,
    csrf_manager: ActorHandle,
}

impl std::fmt::Debug for CsrfLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CsrfLayer")
            .field("config", &self.config)
            .field("csrf_manager", &"ActorHandle")
            .finish()
    }
}

impl CsrfLayer {
    /// Create new CSRF layer with CSRF manager from state
    #[must_use]
    pub fn new(state: &ActonHtmxState) -> Self {
        Self {
            config: CsrfConfig::default(),
            csrf_manager: state.csrf_manager().clone(),
        }
    }

    /// Create CSRF layer with custom configuration
    #[must_use]
    pub fn with_config(state: &ActonHtmxState, config: CsrfConfig) -> Self {
        Self {
            config,
            csrf_manager: state.csrf_manager().clone(),
        }
    }

    /// Create CSRF layer from an existing agent handle
    #[must_use]
    pub fn from_handle(csrf_manager: ActorHandle) -> Self {
        Self {
            config: CsrfConfig::default(),
            csrf_manager,
        }
    }

    /// Create CSRF layer from handle with custom configuration
    #[must_use]
    pub const fn from_handle_with_config(csrf_manager: ActorHandle, config: CsrfConfig) -> Self {
        Self {
            config,
            csrf_manager,
        }
    }
}

impl<S> Layer<S> for CsrfLayer {
    type Service = CsrfMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CsrfMiddleware {
            inner,
            config: Arc::new(self.config.clone()),
            csrf_manager: self.csrf_manager.clone(),
        }
    }
}

/// CSRF middleware that validates tokens on state-changing requests
///
/// Automatically validates CSRF tokens from the `CsrfManagerAgent` on
/// POST, PUT, DELETE, and PATCH requests.
#[derive(Clone)]
pub struct CsrfMiddleware<S> {
    inner: S,
    config: Arc<CsrfConfig>,
    csrf_manager: ActorHandle,
}

impl<S: std::fmt::Debug> std::fmt::Debug for CsrfMiddleware<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CsrfMiddleware")
            .field("inner", &self.inner)
            .field("config", &self.config)
            .field("csrf_manager", &"ActorHandle")
            .finish()
    }
}

impl<S> Service<Request> for CsrfMiddleware<S>
where
    S: Service<Request, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let config = self.config.clone();
        let csrf_manager = self.csrf_manager.clone();
        let mut inner = self.inner.clone();
        let timeout = Duration::from_millis(config.agent_timeout_ms);

        // Skip CSRF validation for idempotent methods
        if is_method_safe(req.method()) {
            return Box::pin(inner.call(req));
        }

        // Skip CSRF validation for configured paths
        let path = req.uri().path().to_string();
        if config.skip_paths.iter().any(|skip| skip == &path) {
            return Box::pin(inner.call(req));
        }

        // Get session ID from request extensions (set by SessionMiddleware)
        let Some(session_id) = req.extensions().get::<SessionId>().cloned() else {
            tracing::warn!("CSRF middleware requires SessionMiddleware to be applied first");
            return Box::pin(async move {
                Ok(csrf_validation_error(
                    "Session not found - ensure SessionMiddleware is applied",
                ))
            });
        };

        // Extract CSRF token from request header
        let Some(token) = extract_csrf_token(&req, &config) else {
            let method = req.method().clone();
            tracing::warn!("CSRF token missing for {} {}", method, path);
            return Box::pin(async move { Ok(csrf_validation_error("CSRF token missing")) });
        };

        Box::pin(async move {
            // Validate token with CSRF manager
            let (validate_request, rx) = ValidateToken::new(session_id, token);
            csrf_manager.send(validate_request).await;

            let is_valid = match tokio::time::timeout(timeout, rx).await {
                Ok(Ok(valid)) => valid,
                Ok(Err(_)) => {
                    tracing::error!("CSRF validation channel error");
                    false
                }
                Err(_) => {
                    tracing::error!("CSRF validation timeout");
                    false
                }
            };

            if !is_valid {
                tracing::warn!("CSRF token validation failed");
                return Ok(csrf_validation_error("CSRF token validation failed"));
            }

            // Token validated - proceed with request
            inner.call(req).await
        })
    }
}

/// Check if HTTP method is considered safe (doesn't modify state)
const fn is_method_safe(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

/// Extract CSRF token from request (header or form data)
fn extract_csrf_token(req: &Request, config: &CsrfConfig) -> Option<CsrfToken> {
    // First, try to get token from header
    if let Some(token_value) = req.headers().get(&config.header_name) {
        if let Ok(token_str) = token_value.to_str() {
            return Some(CsrfToken::from_string(token_str.to_string()));
        }
    }

    // If not in header, check if it's form data
    // Note: This is a simplified implementation. In production, you'd want to
    // properly parse form data without consuming the body.
    // For now, we'll just check the header.

    None
}

/// Create a 403 Forbidden response for CSRF validation failure
fn csrf_validation_error(message: &str) -> Response<Body> {
    let body = if cfg!(debug_assertions) {
        // In development, provide detailed error message
        format!("CSRF validation failed: {message}")
    } else {
        // In production, use generic error message
        "Forbidden".to_string()
    };

    (StatusCode::FORBIDDEN, body).into_response()
}

// =============================================================================
// Microservices CSRF Middleware
// =============================================================================
//
// Alternative CSRF middleware implementation that uses the auth-service via gRPC
// for token validation instead of the local CsrfManagerAgent.

/// Layer for microservices-based CSRF middleware
///
/// Uses the auth-service for CSRF token validation instead of the local agent.
/// Requires the `microservices` feature.
#[cfg(feature = "microservices")]
#[derive(Clone)]
pub struct MicroservicesCsrfLayer {
    config: CsrfConfig,
    services: crate::htmx::clients::ServiceRegistry,
}

#[cfg(feature = "microservices")]
impl std::fmt::Debug for MicroservicesCsrfLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroservicesCsrfLayer")
            .field("config", &self.config)
            .field("services", &"ServiceRegistry")
            .finish()
    }
}

#[cfg(feature = "microservices")]
impl MicroservicesCsrfLayer {
    /// Create new microservices CSRF layer from application state
    ///
    /// # Errors
    ///
    /// Returns error if the auth service is not configured in the registry.
    pub fn new(state: &ActonHtmxState) -> Result<Self, crate::htmx::clients::ClientError> {
        let services = state
            .services()
            .ok_or(crate::htmx::clients::ClientError::NotConfigured(
                "services registry",
            ))?
            .clone();

        // Validate auth service is available
        let _ = services.auth()?;

        Ok(Self {
            config: CsrfConfig::default(),
            services,
        })
    }

    /// Create microservices CSRF layer with custom configuration
    ///
    /// # Errors
    ///
    /// Returns error if the auth service is not configured in the registry.
    pub fn with_config(
        state: &ActonHtmxState,
        config: CsrfConfig,
    ) -> Result<Self, crate::htmx::clients::ClientError> {
        let services = state
            .services()
            .ok_or(crate::htmx::clients::ClientError::NotConfigured(
                "services registry",
            ))?
            .clone();

        // Validate auth service is available
        let _ = services.auth()?;

        Ok(Self { config, services })
    }

    /// Create microservices CSRF layer from service registry
    ///
    /// # Errors
    ///
    /// Returns error if the auth service is not configured in the registry.
    pub fn from_registry(
        services: crate::htmx::clients::ServiceRegistry,
    ) -> Result<Self, crate::htmx::clients::ClientError> {
        // Validate auth service is available
        let _ = services.auth()?;

        Ok(Self {
            config: CsrfConfig::default(),
            services,
        })
    }

    /// Create microservices CSRF layer from registry with custom configuration
    ///
    /// # Errors
    ///
    /// Returns error if the auth service is not configured in the registry.
    pub fn from_registry_with_config(
        services: crate::htmx::clients::ServiceRegistry,
        config: CsrfConfig,
    ) -> Result<Self, crate::htmx::clients::ClientError> {
        // Validate auth service is available
        let _ = services.auth()?;

        Ok(Self { config, services })
    }
}

#[cfg(feature = "microservices")]
impl<S> Layer<S> for MicroservicesCsrfLayer {
    type Service = MicroservicesCsrfMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MicroservicesCsrfMiddleware {
            inner,
            config: Arc::new(self.config.clone()),
            services: self.services.clone(),
        }
    }
}

/// CSRF middleware that validates tokens via the auth-service
///
/// Uses gRPC calls to the auth-service for CSRF token validation instead
/// of local agent communication. Provides graceful fallback on service
/// unavailability.
#[cfg(feature = "microservices")]
#[derive(Clone)]
pub struct MicroservicesCsrfMiddleware<S> {
    inner: S,
    config: Arc<CsrfConfig>,
    services: crate::htmx::clients::ServiceRegistry,
}

#[cfg(feature = "microservices")]
impl<S: std::fmt::Debug> std::fmt::Debug for MicroservicesCsrfMiddleware<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroservicesCsrfMiddleware")
            .field("inner", &self.inner)
            .field("config", &self.config)
            .field("services", &"ServiceRegistry")
            .finish()
    }
}

#[cfg(feature = "microservices")]
impl<S> Service<Request> for MicroservicesCsrfMiddleware<S>
where
    S: Service<Request, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let config = self.config.clone();
        let services = self.services.clone();
        let mut inner = self.inner.clone();
        let timeout = Duration::from_millis(config.agent_timeout_ms);

        // Skip CSRF validation for idempotent methods
        if is_method_safe(req.method()) {
            return Box::pin(inner.call(req));
        }

        // Skip CSRF validation for configured paths
        let path = req.uri().path().to_string();
        if config.skip_paths.iter().any(|skip| skip == &path) {
            return Box::pin(inner.call(req));
        }

        // Get session ID from request extensions (set by SessionMiddleware)
        let Some(session_id) = req.extensions().get::<SessionId>().cloned() else {
            tracing::warn!("CSRF middleware requires SessionMiddleware to be applied first");
            return Box::pin(async move {
                Ok(csrf_validation_error(
                    "Session not found - ensure SessionMiddleware is applied",
                ))
            });
        };

        // Extract CSRF token from request header
        let Some(token) = extract_csrf_token(&req, &config) else {
            let method = req.method().clone();
            tracing::warn!("CSRF token missing for {} {}", method, path);
            return Box::pin(async move { Ok(csrf_validation_error("CSRF token missing")) });
        };

        Box::pin(async move {
            // Validate token with auth service
            let is_valid = match services.auth() {
                Ok(auth_client) => {
                    let validation_result = tokio::time::timeout(timeout, async {
                        let mut client = auth_client.write().await;
                        client
                            .validate_csrf_token(session_id.as_str(), token.as_str())
                            .await
                    })
                    .await;

                    match validation_result {
                        Ok(Ok(valid)) => valid,
                        Ok(Err(e)) => {
                            tracing::error!("CSRF validation service error: {}", e);
                            false
                        }
                        Err(_) => {
                            tracing::error!("CSRF validation timeout");
                            false
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Auth service unavailable for CSRF validation: {}", e);
                    // Service unavailable - fail closed for security
                    false
                }
            };

            if !is_valid {
                tracing::warn!("CSRF token validation failed");
                return Ok(csrf_validation_error("CSRF token validation failed"));
            }

            // Token validated - proceed with request
            inner.call(req).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csrf_config_default() {
        let config = CsrfConfig::default();
        assert_eq!(config.header_name, CSRF_HEADER_NAME);
        assert_eq!(config.form_field, CSRF_FORM_FIELD);
        assert_eq!(config.agent_timeout_ms, 100);
        assert!(config.skip_paths.is_empty());
    }

    #[test]
    fn test_csrf_config_skip_path() {
        let config = CsrfConfig::new().skip_path("/webhooks/github");
        assert_eq!(config.skip_paths.len(), 1);
        assert_eq!(config.skip_paths[0], "/webhooks/github");
    }

    #[test]
    fn test_csrf_config_skip_paths() {
        let config = CsrfConfig::new().skip_paths(vec![
            "/health".to_string(),
            "/metrics".to_string(),
        ]);
        assert_eq!(config.skip_paths.len(), 2);
        assert!(config.skip_paths.contains(&"/health".to_string()));
        assert!(config.skip_paths.contains(&"/metrics".to_string()));
    }

    #[test]
    fn test_is_method_safe() {
        assert!(is_method_safe(&Method::GET));
        assert!(is_method_safe(&Method::HEAD));
        assert!(is_method_safe(&Method::OPTIONS));
        assert!(is_method_safe(&Method::TRACE));

        assert!(!is_method_safe(&Method::POST));
        assert!(!is_method_safe(&Method::PUT));
        assert!(!is_method_safe(&Method::DELETE));
        assert!(!is_method_safe(&Method::PATCH));
    }
}

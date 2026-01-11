//! Session middleware for automatic session management
//!
//! Provides middleware that handles session cookie extraction, validation,
//! and persistence across requests. Integrates with the `SessionManagerAgent`
//! for session storage.

use crate::htmx::agents::{LoadSession, SaveSession};
use crate::htmx::auth::session::{SessionData, SessionId};
use crate::htmx::state::ActonHtmxState;
use acton_reactive::prelude::{ActorHandle, ActorHandleInterface};
use axum::{
    body::Body,
    extract::Request,
    http::header::{COOKIE, SET_COOKIE},
    response::Response,
};
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Layer, Service};

/// Session cookie name
pub const SESSION_COOKIE_NAME: &str = "acton_session";

/// Session configuration for middleware
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Cookie name for session ID
    pub cookie_name: String,
    /// Cookie path
    pub cookie_path: String,
    /// HTTP-only cookie (recommended: true)
    pub http_only: bool,
    /// Secure cookie (HTTPS only)
    pub secure: bool,
    /// SameSite policy
    pub same_site: SameSite,
    /// Session TTL in seconds
    pub max_age_secs: u64,
    /// Timeout for agent communication in milliseconds
    pub agent_timeout_ms: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cookie_name: SESSION_COOKIE_NAME.to_string(),
            cookie_path: "/".to_string(),
            http_only: true,
            secure: !cfg!(debug_assertions),
            same_site: SameSite::Lax,
            max_age_secs: 86400, // 24 hours
            agent_timeout_ms: 100,
        }
    }
}

/// SameSite cookie policy
#[derive(Clone, Copy, Debug, Default)]
pub enum SameSite {
    /// Strict same-site policy
    Strict,
    /// Lax same-site policy (recommended)
    #[default]
    Lax,
    /// No same-site restriction (requires Secure)
    None,
}

impl SameSite {
    /// Convert to cookie attribute string
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "Strict",
            Self::Lax => "Lax",
            Self::None => "None",
        }
    }
}

/// Layer for session middleware
///
/// Requires `ActonHtmxState` to be present in the request extensions,
/// typically added via `.with_state()`.
#[derive(Clone)]
pub struct SessionLayer {
    config: SessionConfig,
    session_manager: ActorHandle,
}

impl std::fmt::Debug for SessionLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionLayer")
            .field("config", &self.config)
            .field("session_manager", &"ActorHandle")
            .finish()
    }
}

impl SessionLayer {
    /// Create new session layer with session manager from state
    #[must_use]
    pub fn new(state: &ActonHtmxState) -> Self {
        Self {
            config: SessionConfig::default(),
            session_manager: state.session_manager().clone(),
        }
    }

    /// Create session layer with custom configuration
    #[must_use]
    pub fn with_config(state: &ActonHtmxState, config: SessionConfig) -> Self {
        Self {
            config,
            session_manager: state.session_manager().clone(),
        }
    }

    /// Create session layer from an existing agent handle
    #[must_use]
    pub fn from_handle(session_manager: ActorHandle) -> Self {
        Self {
            config: SessionConfig::default(),
            session_manager,
        }
    }
}

impl<S> Layer<S> for SessionLayer {
    type Service = SessionMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware {
            inner,
            config: Arc::new(self.config.clone()),
            session_manager: self.session_manager.clone(),
        }
    }
}

/// Session middleware that handles cookie-based sessions
///
/// Automatically loads sessions from the `SessionManagerAgent` on request
/// and saves modified sessions on response.
#[derive(Clone)]
pub struct SessionMiddleware<S> {
    inner: S,
    config: Arc<SessionConfig>,
    session_manager: ActorHandle,
}

impl<S: std::fmt::Debug> std::fmt::Debug for SessionMiddleware<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionMiddleware")
            .field("inner", &self.inner)
            .field("config", &self.config)
            .field("session_manager", &"ActorHandle")
            .finish()
    }
}

impl<S> Service<Request> for SessionMiddleware<S>
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

    fn call(&mut self, mut req: Request) -> Self::Future {
        let config = self.config.clone();
        let session_manager = self.session_manager.clone();
        let mut inner = self.inner.clone();
        let timeout = Duration::from_millis(config.agent_timeout_ms);

        Box::pin(async move {
            // Extract session ID from cookie
            let existing_session_id = extract_session_id(&req, &config.cookie_name);

            // Load or create session
            let (session_id, session_data, is_new) = if let Some(id) = existing_session_id {
                // Try to load existing session from agent
                let (request, rx) = LoadSession::with_response(id.clone());
                session_manager.send(request).await;

                // Wait for response with timeout
                if let Ok(Ok(Some(data))) = tokio::time::timeout(timeout, rx).await {
                    (id, data, false)
                } else {
                    // Session not found or timeout - create new session
                    let new_id = SessionId::generate();
                    (new_id, SessionData::new(), true)
                }
            } else {
                // No session cookie - create new session
                let id = SessionId::generate();
                (id, SessionData::new(), true)
            };

            // Insert session into request extensions for handlers to access
            req.extensions_mut().insert(session_id.clone());
            req.extensions_mut().insert(session_data.clone());

            // Call inner service
            let mut response = inner.call(req).await?;

            // Get potentially modified session data from response extensions
            // (handlers can modify it via SessionExtractor)
            let final_session_data = response
                .extensions()
                .get::<SessionData>()
                .cloned()
                .unwrap_or(session_data);

            // Save session to agent (fire-and-forget for performance)
            let save_request = SaveSession::new(session_id.clone(), final_session_data);
            session_manager.send(save_request).await;

            // Set session cookie if new
            if is_new {
                set_session_cookie(&mut response, &session_id, &config);
            }

            Ok(response)
        })
    }
}

/// Extract session ID from request cookies
fn extract_session_id(req: &Request, cookie_name: &str) -> Option<SessionId> {
    let cookie_header = req.headers().get(COOKIE)?;
    let cookie_str = cookie_header.to_str().ok()?;

    // Parse cookies looking for our session cookie
    for cookie in cookie_str.split(';') {
        let cookie = cookie.trim();
        if let Some((name, value)) = cookie.split_once('=') {
            if name.trim() == cookie_name {
                return SessionId::from_str(value.trim()).ok();
            }
        }
    }

    None
}

/// Set session cookie on response
fn set_session_cookie(
    response: &mut Response<Body>,
    session_id: &SessionId,
    config: &SessionConfig,
) {
    let mut cookie_value = format!(
        "{}={}; Path={}; Max-Age={}; SameSite={}",
        config.cookie_name,
        session_id.as_str(),
        config.cookie_path,
        config.max_age_secs,
        config.same_site.as_str()
    );

    if config.http_only {
        cookie_value.push_str("; HttpOnly");
    }

    if config.secure {
        cookie_value.push_str("; Secure");
    }

    if let Ok(header_value) = cookie_value.parse() {
        response.headers_mut().append(SET_COOKIE, header_value);
    }
}

// =============================================================================
// Microservices Session Middleware
// =============================================================================

/// Layer for microservices-based session middleware
///
/// Uses the auth-service via gRPC for session management instead of
/// the local `SessionManagerAgent`.
#[cfg(feature = "microservices")]
#[derive(Clone)]
pub struct MicroservicesSessionLayer {
    config: SessionConfig,
    services: crate::htmx::clients::ServiceRegistry,
}

#[cfg(feature = "microservices")]
impl std::fmt::Debug for MicroservicesSessionLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroservicesSessionLayer")
            .field("config", &self.config)
            .field("services", &"ServiceRegistry")
            .finish()
    }
}

#[cfg(feature = "microservices")]
impl MicroservicesSessionLayer {
    /// Create a new microservices session layer
    ///
    /// # Errors
    ///
    /// Returns error if auth service is not configured in the registry.
    pub fn new(
        state: &ActonHtmxState,
    ) -> Result<Self, crate::htmx::clients::ClientError> {
        let services = state
            .services()
            .ok_or(crate::htmx::clients::ClientError::NotConfigured(
                "services registry",
            ))?
            .clone();

        // Verify auth service is available
        let _ = services.auth()?;

        Ok(Self {
            config: SessionConfig::default(),
            services,
        })
    }

    /// Create with custom configuration
    ///
    /// # Errors
    ///
    /// Returns error if auth service is not configured in the registry.
    pub fn with_config(
        state: &ActonHtmxState,
        config: SessionConfig,
    ) -> Result<Self, crate::htmx::clients::ClientError> {
        let services = state
            .services()
            .ok_or(crate::htmx::clients::ClientError::NotConfigured(
                "services registry",
            ))?
            .clone();

        // Verify auth service is available
        let _ = services.auth()?;

        Ok(Self { config, services })
    }

    /// Create from a service registry directly
    ///
    /// # Errors
    ///
    /// Returns error if auth service is not configured in the registry.
    pub fn from_registry(
        services: crate::htmx::clients::ServiceRegistry,
    ) -> Result<Self, crate::htmx::clients::ClientError> {
        // Verify auth service is available
        let _ = services.auth()?;

        Ok(Self {
            config: SessionConfig::default(),
            services,
        })
    }
}

#[cfg(feature = "microservices")]
impl<S> Layer<S> for MicroservicesSessionLayer {
    type Service = MicroservicesSessionMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MicroservicesSessionMiddleware {
            inner,
            config: Arc::new(self.config.clone()),
            services: self.services.clone(),
        }
    }
}

/// Microservices-based session middleware
///
/// Uses the auth-service via gRPC for session operations.
#[cfg(feature = "microservices")]
#[derive(Clone)]
pub struct MicroservicesSessionMiddleware<S> {
    inner: S,
    config: Arc<SessionConfig>,
    services: crate::htmx::clients::ServiceRegistry,
}

#[cfg(feature = "microservices")]
impl<S: std::fmt::Debug> std::fmt::Debug for MicroservicesSessionMiddleware<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroservicesSessionMiddleware")
            .field("inner", &self.inner)
            .field("config", &self.config)
            .field("services", &"ServiceRegistry")
            .finish()
    }
}

#[cfg(feature = "microservices")]
impl<S> Service<Request> for MicroservicesSessionMiddleware<S>
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

    fn call(&mut self, mut req: Request) -> Self::Future {
        let config = self.config.clone();
        let services = self.services.clone();
        let mut inner = self.inner.clone();
        let timeout_duration = Duration::from_millis(config.agent_timeout_ms);

        Box::pin(async move {
            // Extract session ID from cookie
            let existing_session_id = extract_session_id(&req, &config.cookie_name);

            // Load or create session via auth-service
            let (session_id, session_data, is_new) = load_or_create_session_via_service(
                &services,
                existing_session_id,
                timeout_duration,
                i64::try_from(config.max_age_secs).unwrap_or(86400),
            )
            .await
            .unwrap_or_else(|_| {
                // Service unavailable - create local session as fallback
                let id = SessionId::generate();
                (id, SessionData::new(), true)
            });

            // Insert session into request extensions for handlers to access
            req.extensions_mut().insert(session_id.clone());
            req.extensions_mut().insert(session_data.clone());

            // Call inner service
            let mut response = inner.call(req).await?;

            // Get potentially modified session data from response extensions
            let final_session_data = response
                .extensions()
                .get::<SessionData>()
                .cloned()
                .unwrap_or(session_data);

            // Save session to auth-service (fire-and-forget for performance)
            let _ = save_session_via_service(
                &services,
                &session_id,
                &final_session_data,
                timeout_duration,
            )
            .await;

            // Set session cookie if new
            if is_new {
                set_session_cookie(&mut response, &session_id, &config);
            }

            Ok(response)
        })
    }
}

/// Load or create a session via the auth-service
#[cfg(feature = "microservices")]
async fn load_or_create_session_via_service(
    services: &crate::htmx::clients::ServiceRegistry,
    existing_session_id: Option<SessionId>,
    timeout: Duration,
    ttl_seconds: i64,
) -> Result<(SessionId, SessionData, bool), crate::htmx::clients::ClientError> {
    let auth = services.auth()?;

    if let Some(id) = existing_session_id {
        // Try to validate existing session
        let validate_result = tokio::time::timeout(timeout, async {
            let mut client = auth.write().await;
            client.validate_session(id.as_str()).await
        })
        .await;

        if let Ok(Ok(Some(proto_session))) = validate_result {
            // Convert proto session to local SessionData
            let session_data = proto_session_to_session_data(&proto_session);
            return Ok((id, session_data, false));
        }
    }

    // Create new session via auth-service
    let create_result = tokio::time::timeout(timeout, async {
        let mut client = auth.write().await;
        client
            .create_session(None, ttl_seconds, std::collections::HashMap::new())
            .await
    })
    .await;

    match create_result {
        Ok(Ok(proto_session)) => {
            let session_id = SessionId::from_str(&proto_session.session_id)
                .unwrap_or_else(|_| SessionId::generate());
            let session_data = proto_session_to_session_data(&proto_session);
            Ok((session_id, session_data, true))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(crate::htmx::clients::ClientError::RequestFailed(
            "timeout".to_string(),
        )),
    }
}

/// Save session via the auth-service
#[cfg(feature = "microservices")]
async fn save_session_via_service(
    services: &crate::htmx::clients::ServiceRegistry,
    session_id: &SessionId,
    session_data: &SessionData,
    timeout: Duration,
) -> Result<(), crate::htmx::clients::ClientError> {
    let auth = services.auth()?;

    // Convert SessionData to HashMap for update
    let data = session_data_to_hashmap(session_data);

    let _ = tokio::time::timeout(timeout, async {
        let mut client = auth.write().await;
        client
            .update_session(session_id.as_str(), data, session_data.user_id)
            .await
    })
    .await;

    Ok(())
}

/// Convert proto Session to local SessionData
#[cfg(feature = "microservices")]
fn proto_session_to_session_data(
    proto: &crate::htmx::clients::Session,
) -> SessionData {
    let mut session_data = SessionData::new();

    // Set user_id directly
    session_data.user_id = proto.user_id;

    // Set user_name directly if available
    if let Some(ref name) = proto.user_name {
        session_data.user_name = Some(name.clone());
    }

    // Store user_email in custom data if available
    if let Some(ref email) = proto.user_email {
        let _ = session_data.set("user_email".to_string(), email.clone());
    }

    // Copy additional data fields
    for (key, value) in &proto.data {
        let _ = session_data.set(key.clone(), value.clone());
    }

    session_data
}

/// Convert SessionData to HashMap for proto
#[cfg(feature = "microservices")]
fn session_data_to_hashmap(session_data: &SessionData) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();

    // Add user_name if present
    if let Some(ref name) = session_data.user_name {
        map.insert("user_name".to_string(), name.clone());
    }

    // Add all custom data from session
    for (key, value) in &session_data.data {
        if let Some(s) = value.as_str() {
            map.insert(key.clone(), s.to_string());
        } else if let Ok(s) = serde_json::to_string(value) {
            map.insert(key.clone(), s);
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert_eq!(config.cookie_name, SESSION_COOKIE_NAME);
        assert!(config.http_only);
        assert_eq!(config.max_age_secs, 86400);
    }

    #[test]
    fn test_same_site_as_str() {
        assert_eq!(SameSite::Strict.as_str(), "Strict");
        assert_eq!(SameSite::Lax.as_str(), "Lax");
        assert_eq!(SameSite::None.as_str(), "None");
    }
}

//! Session middleware for automatic session management
//!
//! Provides middleware that handles session cookie extraction, validation,
//! and persistence across requests. Integrates with the `SessionManagerAgent`
//! for session storage.

use crate::agents::{LoadSession, SaveSession};
use crate::auth::session::{SessionData, SessionId};
use crate::state::ActonHtmxState;
use acton_reactive::prelude::{AgentHandle, AgentHandleInterface};
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
    session_manager: AgentHandle,
}

impl std::fmt::Debug for SessionLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionLayer")
            .field("config", &self.config)
            .field("session_manager", &"AgentHandle")
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
    pub fn from_handle(session_manager: AgentHandle) -> Self {
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
    session_manager: AgentHandle,
}

impl<S: std::fmt::Debug> std::fmt::Debug for SessionMiddleware<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionMiddleware")
            .field("inner", &self.inner)
            .field("config", &self.config)
            .field("session_manager", &"AgentHandle")
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

//! Session middleware for automatic session management
//!
//! Provides middleware that handles session cookie extraction, validation,
//! and persistence across requests.

#![allow(dead_code)]

use crate::auth::session::{SessionData, SessionId};
use axum::{
    body::Body,
    extract::Request,
    http::header::{COOKIE, SET_COOKIE},
    response::Response,
};
use std::str::FromStr;
use std::task::{Context, Poll};
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
#[derive(Clone, Debug)]
pub struct SessionLayer {
    config: SessionConfig,
}

impl SessionLayer {
    /// Create new session layer with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SessionConfig::default(),
        }
    }

    /// Create session layer with custom configuration
    #[must_use]
    pub const fn with_config(config: SessionConfig) -> Self {
        Self { config }
    }
}

impl Default for SessionLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for SessionLayer {
    type Service = SessionMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionMiddleware {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Session middleware that handles cookie-based sessions
#[derive(Clone, Debug)]
pub struct SessionMiddleware<S> {
    inner: S,
    config: SessionConfig,
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
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Extract session ID from cookie
            let session_id = extract_session_id(&req, &config.cookie_name);

            // Create or load session
            let (session_id, session_data, is_new) = session_id.map_or_else(
                || {
                    // Generate new session
                    let id = SessionId::generate();
                    (id, SessionData::new(), true)
                },
                |id| {
                    // TODO: Load from SessionManagerAgent
                    // For now, create new session data
                    (id, SessionData::new(), false)
                },
            );

            // Insert session into request extensions for handlers to access
            req.extensions_mut().insert(session_id.clone());
            req.extensions_mut().insert(session_data);

            // Call inner service
            let mut response = inner.call(req).await?;

            // Set session cookie if new or modified
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
fn set_session_cookie(response: &mut Response<Body>, session_id: &SessionId, config: &SessionConfig) {
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

//! Auth service for Acton DX.
//!
//! Provides session management, password hashing, and CSRF protection.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod agents;
pub mod config;
pub mod services;

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Session data stored in the session manager.
#[derive(Debug, Clone)]
pub struct SessionData {
    /// Unique session identifier.
    pub session_id: String,
    /// Associated user ID (if authenticated).
    pub user_id: Option<i64>,
    /// User email (if authenticated).
    pub user_email: Option<String>,
    /// User display name (if authenticated).
    pub user_name: Option<String>,
    /// Arbitrary session data.
    pub data: HashMap<String, String>,
    /// Flash messages for one-time display.
    pub flash_messages: Vec<FlashMessage>,
    /// CSRF token for this session.
    pub csrf_token: String,
    /// Session creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Session expiration timestamp.
    pub expires_at: DateTime<Utc>,
}

impl SessionData {
    /// Create a new session with the given TTL.
    #[must_use]
    pub fn new(ttl_seconds: u64, user_id: Option<i64>) -> Self {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        use rand::Rng;

        let mut session_id_bytes = [0u8; 32];
        let mut csrf_bytes = [0u8; 32];
        rand::rng().fill(&mut session_id_bytes);
        rand::rng().fill(&mut csrf_bytes);

        let now = Utc::now();
        let ttl = chrono::Duration::seconds(i64::try_from(ttl_seconds).unwrap_or(i64::MAX));

        Self {
            session_id: URL_SAFE_NO_PAD.encode(session_id_bytes),
            user_id,
            user_email: None,
            user_name: None,
            data: HashMap::new(),
            flash_messages: Vec::new(),
            csrf_token: URL_SAFE_NO_PAD.encode(csrf_bytes),
            created_at: now,
            expires_at: now + ttl,
        }
    }

    /// Check if the session has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

/// Flash message for one-time display.
#[derive(Debug, Clone)]
pub struct FlashMessage {
    /// Message level (e.g., "success", "error", "info", "warning").
    pub level: String,
    /// Message content.
    pub message: String,
}

// Re-export key types for convenience
pub use agents::SessionManagerAgent;
pub use config::AuthServiceConfig;
pub use services::{CsrfServiceImpl, PasswordServiceImpl, SessionServiceImpl};

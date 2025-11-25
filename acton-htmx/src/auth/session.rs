//! Session types and data structures
//!
//! This module provides the core session types used throughout the framework.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique session identifier
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Generate a new cryptographically secure session ID
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from a string (validates format)
    ///
    /// # Errors
    ///
    /// Returns error if the string is not a valid UUID
    pub fn try_from_string(s: String) -> Result<Self, SessionError> {
        Uuid::parse_str(&s)
            .map(|_| Self(s))
            .map_err(|_| SessionError::InvalidSessionId)
    }

    /// Get the session ID as a string reference
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = SessionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_string(s.to_string())
    }
}

/// Session data stored per-session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// When this session was created
    pub created_at: DateTime<Utc>,
    /// When this session was last accessed
    pub last_accessed: DateTime<Utc>,
    /// When this session expires
    pub expires_at: DateTime<Utc>,
    /// User ID (if authenticated)
    pub user_id: Option<i64>,
    /// Custom session data (key-value store)
    pub data: HashMap<String, serde_json::Value>,
    /// Flash messages queued for next request
    pub flash_messages: Vec<FlashMessage>,
}

impl SessionData {
    /// Create new session data with default expiration (24 hours)
    #[must_use]
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            last_accessed: now,
            expires_at: now + Duration::hours(24),
            user_id: None,
            data: HashMap::new(),
            flash_messages: Vec::new(),
        }
    }

    /// Create session with custom expiration duration
    #[must_use]
    pub fn with_expiration(duration: Duration) -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            last_accessed: now,
            expires_at: now + duration,
            user_id: None,
            data: HashMap::new(),
            flash_messages: Vec::new(),
        }
    }

    /// Check if session is expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Update last accessed time and extend expiration
    pub fn touch(&mut self, extend_by: Duration) {
        self.last_accessed = Utc::now();
        self.expires_at = self.last_accessed + extend_by;
    }

    /// Validate session is not expired and touch it if valid
    ///
    /// This method combines the common pattern of checking expiry and
    /// extending the session lifetime in a single operation.
    ///
    /// # Returns
    ///
    /// - `true` if the session is valid (not expired) - session is touched
    /// - `false` if the session has expired - session is not modified
    ///
    /// # Example
    ///
    /// ```
    /// use acton_htmx::auth::session::SessionData;
    /// use chrono::Duration;
    ///
    /// let mut session = SessionData::new();
    /// assert!(session.validate_and_touch(Duration::hours(24)));
    ///
    /// // Expired session returns false
    /// let mut expired = SessionData::with_expiration(Duration::seconds(-1));
    /// assert!(!expired.validate_and_touch(Duration::hours(24)));
    /// ```
    pub fn validate_and_touch(&mut self, extend_by: Duration) -> bool {
        if self.is_expired() {
            false
        } else {
            self.touch(extend_by);
            true
        }
    }

    /// Get a value from session data
    #[must_use]
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set a value in session data
    ///
    /// # Errors
    ///
    /// Returns error if value cannot be serialized to JSON
    pub fn set<T: Serialize>(&mut self, key: String, value: T) -> Result<(), SessionError> {
        let json_value = serde_json::to_value(value)?;
        self.data.insert(key, json_value);
        Ok(())
    }

    /// Remove a value from session data
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.data.remove(key)
    }

    /// Clear all session data (keeps metadata)
    pub fn clear(&mut self) {
        self.data.clear();
        self.flash_messages.clear();
        self.user_id = None;
    }
}

impl Default for SessionData {
    fn default() -> Self {
        Self::new()
    }
}

/// Flash message for one-time display
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlashMessage {
    /// Message level (success, info, warning, error)
    pub level: FlashLevel,
    /// Message text
    pub message: String,
    /// Optional title
    pub title: Option<String>,
}

impl FlashMessage {
    /// Create a success flash message
    #[must_use]
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Success,
            message: message.into(),
            title: None,
        }
    }

    /// Create an info flash message
    #[must_use]
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Info,
            message: message.into(),
            title: None,
        }
    }

    /// Create a warning flash message
    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Warning,
            message: message.into(),
            title: None,
        }
    }

    /// Create an error flash message
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Error,
            message: message.into(),
            title: None,
        }
    }

    /// Set the title for this flash message
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Get CSS class for this flash level
    #[must_use]
    pub const fn css_class(&self) -> &'static str {
        self.level.css_class()
    }
}

/// Flash message severity level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlashLevel {
    /// Success message (green)
    Success,
    /// Informational message (blue)
    Info,
    /// Warning message (yellow)
    Warning,
    /// Error message (red)
    Error,
}

impl FlashLevel {
    /// Get CSS class for this level
    #[must_use]
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Success => "flash-success",
            Self::Info => "flash-info",
            Self::Warning => "flash-warning",
            Self::Error => "flash-error",
        }
    }
}

/// Session-related errors
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// Invalid session ID format
    #[error("Invalid session ID")]
    InvalidSessionId,

    /// Session not found
    #[error("Session not found")]
    NotFound,

    /// Session expired
    #[error("Session expired")]
    Expired,

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Redis error
    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(String),

    /// Agent communication error
    #[error("Agent error: {0}")]
    Agent(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generate() {
        let id1 = SessionId::generate();
        let id2 = SessionId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_from_string() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = SessionId::try_from_string(uuid_str.to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_session_id_invalid() {
        let result = SessionId::try_from_string("not-a-uuid".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_session_data_new() {
        let data = SessionData::new();
        assert!(!data.is_expired());
        assert!(data.user_id.is_none());
        assert!(data.data.is_empty());
    }

    #[test]
    fn test_session_data_expiration() {
        let data = SessionData::with_expiration(Duration::seconds(-1));
        assert!(data.is_expired());
    }

    #[test]
    fn test_session_data_touch() {
        let mut data = SessionData::new();
        let original_expiry = data.expires_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        data.touch(Duration::hours(24));
        assert!(data.expires_at > original_expiry);
    }

    #[test]
    fn test_session_data_validate_and_touch_valid() {
        let mut data = SessionData::new();
        let original_expiry = data.expires_at;
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Valid session should return true and extend expiry
        assert!(data.validate_and_touch(Duration::hours(24)));
        assert!(data.expires_at > original_expiry);
    }

    #[test]
    fn test_session_data_validate_and_touch_expired() {
        let mut data = SessionData::with_expiration(Duration::seconds(-1));
        let original_expiry = data.expires_at;

        // Expired session should return false and not modify expiry
        assert!(!data.validate_and_touch(Duration::hours(24)));
        assert_eq!(data.expires_at, original_expiry);
    }

    #[test]
    fn test_session_data_get_set() {
        let mut data = SessionData::new();
        data.set("key".to_string(), "value").unwrap();
        let value: Option<String> = data.get("key");
        assert_eq!(value, Some("value".to_string()));
    }

    #[test]
    fn test_session_data_remove() {
        let mut data = SessionData::new();
        data.set("key".to_string(), "value").unwrap();
        let removed = data.remove("key");
        assert!(removed.is_some());
        let value: Option<String> = data.get("key");
        assert!(value.is_none());
    }

    #[test]
    fn test_flash_message_creation() {
        let flash = FlashMessage::success("Test").with_title("Success");
        assert_eq!(flash.level, FlashLevel::Success);
        assert_eq!(flash.message, "Test");
        assert_eq!(flash.title, Some("Success".to_string()));
    }

    #[test]
    fn test_flash_level_css_class() {
        assert_eq!(FlashLevel::Success.css_class(), "flash-success");
        assert_eq!(FlashLevel::Info.css_class(), "flash-info");
        assert_eq!(FlashLevel::Warning.css_class(), "flash-warning");
        assert_eq!(FlashLevel::Error.css_class(), "flash-error");
    }
}

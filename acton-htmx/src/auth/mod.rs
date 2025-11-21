//! Authentication and session management
//!
//! This module provides session-based authentication with secure HTTP-only cookies.

pub mod session;

pub use session::{FlashLevel, FlashMessage, SessionData, SessionError, SessionId};

use serde::{Deserialize, Serialize};

/// Authenticated user extractor for protected routes
///
/// Use this extractor in handlers that require authentication.
/// The extractor will return a 401 Unauthorized response if no valid session exists.
pub struct Authenticated<T>(std::marker::PhantomData<T>);

/// Optional authentication extractor
///
/// Use this extractor in handlers that work with or without authentication.
/// Returns `Some(user)` if authenticated, `None` otherwise.
pub struct OptionalAuth<T>(std::marker::PhantomData<T>);

/// Session wrapper for handler extractors
///
/// Provides access to the current user's session data within request handlers.
#[derive(Debug, Clone)]
pub struct Session {
    id: SessionId,
    data: SessionData,
}

impl Session {
    /// Create a new session wrapper
    #[must_use]
    pub const fn new(id: SessionId, data: SessionData) -> Self {
        Self { id, data }
    }

    /// Get the session ID
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Get the session data
    #[must_use]
    pub const fn data(&self) -> &SessionData {
        &self.data
    }

    /// Get mutable access to session data
    pub const fn data_mut(&mut self) -> &mut SessionData {
        &mut self.data
    }

    /// Get a value from the session
    #[must_use]
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data.get(key)
    }

    /// Set a value in the session
    ///
    /// # Errors
    ///
    /// Returns error if value cannot be serialized
    pub fn set<T: Serialize>(&mut self, key: String, value: T) -> Result<(), SessionError> {
        self.data.set(key, value)
    }

    /// Remove a value from the session
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.data.remove(key)
    }

    /// Get the user ID if authenticated
    #[must_use]
    pub const fn user_id(&self) -> Option<i64> {
        self.data.user_id
    }

    /// Set the user ID (for login)
    pub const fn set_user_id(&mut self, user_id: Option<i64>) {
        self.data.user_id = user_id;
    }

    /// Add a flash message
    pub fn add_flash(&mut self, message: FlashMessage) {
        self.data.flash_messages.push(message);
    }

    /// Take all flash messages (clears them from session)
    pub fn take_flashes(&mut self) -> Vec<FlashMessage> {
        std::mem::take(&mut self.data.flash_messages)
    }

    /// Check if there are any flash messages
    #[must_use]
    pub fn has_flashes(&self) -> bool {
        !self.data.flash_messages.is_empty()
    }
}

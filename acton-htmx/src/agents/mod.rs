//! acton-reactive agents
//!
//! This module contains actor-based components for background processing,
//! session management, and real-time features.

pub mod session_manager;

// Re-export public types for use by middleware and extractors
pub use session_manager::{
    AddFlash, CleanupExpired, DeleteSession, FlashMessages, GetFlashes, LoadSession, SaveSession,
    SessionLoaded, SessionManagerAgent, SessionNotFound,
};

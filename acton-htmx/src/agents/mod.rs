//! acton-reactive agents
//!
//! This module contains actor-based components for background processing,
//! session management, CSRF protection, and real-time features.

pub mod csrf_manager;
pub mod request_reply;
pub mod session_manager;

// Re-export public types for use by middleware and extractors
pub use csrf_manager::{
    // Web handler messages (oneshot responses)
    CsrfManagerAgent, CsrfToken, DeleteTokenRequest, GetOrCreateTokenRequest, ValidateTokenRequest,
    // Agent-to-agent messages
    CleanupExpired as CsrfCleanupExpired, DeleteToken, GetOrCreateToken, TokenResponse,
    ValidateToken, ValidationResponse,
};
pub use session_manager::{
    // Web handler messages (oneshot responses)
    LoadSessionRequest, ResponseChannel, SaveSessionRequest, TakeFlashesRequest,
    // Agent-to-agent messages
    AddFlash, CleanupExpired, DeleteSession, FlashMessages, GetFlashes, LoadSession, SaveSession,
    SessionLoaded, SessionManagerAgent, SessionNotFound,
};

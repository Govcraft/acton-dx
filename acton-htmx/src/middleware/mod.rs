//! Middleware layers for acton-htmx
//!
//! Provides middleware for:
//! - Session management (cookie-based sessions)
//! - CSRF protection
//! - Security headers
//! - Rate limiting

#![allow(dead_code)]
#![allow(unused_imports)]

pub mod session;

pub use session::{SameSite, SessionConfig, SessionLayer, SessionMiddleware, SESSION_COOKIE_NAME};

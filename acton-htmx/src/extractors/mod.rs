//! Axum extractors for acton-htmx
//!
//! Provides extractors for accessing session data, flash messages,
//! and other request context within handlers.

mod session;

pub use session::{FlashExtractor, OptionalSession, SessionExtractor};

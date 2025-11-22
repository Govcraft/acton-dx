//! Axum extractors for acton-htmx
//!
//! Provides extractors for accessing session data, flash messages,
//! CSRF tokens, validation, file uploads, and other request context within handlers.

mod csrf;
mod file_upload;
mod session;
mod validated;

pub use csrf::CsrfTokenExtractor;
pub use file_upload::{FileUpload, FileUploadError, MultiFileUpload};
pub use session::{FlashExtractor, OptionalSession, SessionExtractor};
pub use validated::{
    format_validation_errors, validation_errors_json, ValidatedForm, ValidationError,
};

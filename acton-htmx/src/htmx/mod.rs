//! HTMX response types and extractors
//!
//! This module builds on `axum-htmx` with additional features:
//! - Out-of-band swaps (`HxSwapOob`)
//! - Automatic template detection (`HxTemplate`)
//! - Smart response enum (`HxResponse`)
//!
//! # Re-exported from axum-htmx
//!
//! All request extractors and response helpers from `axum-htmx` are re-exported
//! for convenience. See [axum-htmx documentation](https://docs.rs/axum-htmx) for
//! detailed usage.
//!
//! # Out-of-Band Swaps
//!
//! Use [`HxSwapOob`] to update multiple page elements in a single response:
//!
//! ```rust,no_run
//! use acton_htmx::htmx::{HxSwapOob, SwapStrategy};
//! use axum::response::Html;
//!
//! async fn update_with_oob() -> impl axum::response::IntoResponse {
//!     let mut oob = HxSwapOob::new();
//!
//!     // Update main content
//!     oob.add("main-content", "<p>New main content</p>", SwapStrategy::InnerHTML);
//!
//!     // Update notification badge
//!     oob.add("notification-count", "<span>5</span>", SwapStrategy::InnerHTML);
//!
//!     // Update flash messages
//!     oob.add("flash-container", r#"<div class="alert">Success!</div>"#, SwapStrategy::InnerHTML);
//!
//!     oob
//! }
//! ```

// Re-export axum-htmx request extractors
pub use axum_htmx::{
    HxBoosted, HxCurrentUrl, HxHistoryRestoreRequest, HxPrompt, HxRequest, HxTarget, HxTrigger,
    HxTriggerName,
};

// Re-export axum-htmx response helpers
pub use axum_htmx::{
    HxLocation, HxPushUrl, HxRedirect, HxRefresh, HxReplaceUrl, HxReselect, HxResponseTrigger,
    HxReswap, HxRetarget,
};

// Re-export axum-htmx middleware and guards
pub use axum_htmx::{AutoVaryLayer, HxRequestGuardLayer};

// acton-htmx extensions
mod swap_oob;
pub use swap_oob::{HxSwapOob, SwapStrategy};

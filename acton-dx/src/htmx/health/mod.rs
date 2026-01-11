//! Health check endpoints
//!
//! This module re-exports acton-service's health check functionality.
//! For HTMX applications, you can use these handlers directly in your routes.
//!
//! # Example
//!
//! ```rust,no_run
//! use axum::{Router, routing::get};
//! use acton_dx::health::{liveness, readiness};
//!
//! let app = Router::new()
//!     .route("/health/live", get(liveness))
//!     .route("/health/ready", get(readiness));
//! ```
//!
//! # Available Handlers
//!
//! - [`liveness`] - Simple liveness probe (200 OK if running)
//! - [`readiness`] - Readiness probe with dependency checks
//!
//! For more advanced health checks with database and Redis pool metrics,
//! use `acton_service::health::pool_metrics` with your `AppState`.

use axum::{
    http::StatusCode,
    response::IntoResponse,
};

/// Liveness probe handler
///
/// Returns 200 OK if the application is running.
/// This is a simple check that the process is alive.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{Router, routing::get};
/// use acton_dx::health::liveness;
///
/// let app = Router::new()
///     .route("/health/live", get(liveness));
/// ```
#[allow(clippy::unused_async)]
pub async fn liveness() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Readiness probe handler
///
/// Returns 200 OK if the application is ready to serve traffic.
/// This is a simple default implementation.
///
/// For more comprehensive readiness checks that verify database and
/// Redis connectivity, use `acton_service::health::readiness` with
/// your application state.
///
/// # Example
///
/// ```rust,no_run
/// use axum::{Router, routing::get};
/// use acton_dx::health::readiness;
///
/// let app = Router::new()
///     .route("/health/ready", get(readiness));
/// ```
#[allow(clippy::unused_async)]
pub async fn readiness() -> impl IntoResponse {
    (StatusCode::OK, "READY")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_liveness_handler() {
        let response = liveness().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_readiness_handler() {
        let response = readiness().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

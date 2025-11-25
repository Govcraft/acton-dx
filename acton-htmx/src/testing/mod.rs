//! Testing utilities for acton-htmx applications
//!
//! This module provides comprehensive test helpers for testing HTMX applications:
//!
//! ## General Testing Utilities
//!
//! - [`TestServer`] - Wrapper around `axum-test` for server testing
//! - [`TestDatabase`] - Helper for SQLx test databases
//! - HTMX assertion helpers for common response patterns
//!
//! ## Agent Testing Utilities
//!
//! - [`agents::AgentTestRuntime`] - Test runtime with automatic cleanup for agent tests
//! - [`agents::await_response`] - Helper to await oneshot responses with timeout
//!
//! ## Domain-Specific Test Utilities
//!
//! - [`MockEmailSender`] - Mock email sender for testing email functionality
//! - [`TestJobQueue`] - In-memory job queue for testing background jobs
//! - [`TestJob`] - Simple test job implementation for testing job execution
//!
//! # Example
//!
//! ```rust,no_run
//! use acton_htmx::testing::{TestServer, TestDatabase, MockEmailSender};
//! use acton_htmx::prelude::*;
//!
//! #[tokio::test]
//! async fn test_login_flow() {
//!     let app = build_test_app().await;
//!     let server = TestServer::new(app).unwrap();
//!
//!     let response = server
//!         .post("/login")
//!         .form(&LoginForm {
//!             email: "test@example.com",
//!             password: "password123",
//!         })
//!         .await;
//!
//!     server.assert_hx_redirect(&response, "/dashboard");
//! }
//! ```

pub mod agents;
pub mod assertions;
pub mod database;
pub mod email;
pub mod jobs;
pub mod server;

// Re-export for convenience
pub use agents::{await_response, await_response_with_timeout, AgentTestRuntime};
pub use assertions::*;
pub use database::TestDatabase;
pub use email::MockEmailSender;
pub use jobs::{
    assert_job_completes_within, assert_job_fails, assert_job_succeeds, TestJob, TestJobQueue,
};
pub use server::TestServer;

// Re-export mockall for test usage
pub use mockall;

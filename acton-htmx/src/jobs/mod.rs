//! Background job processing system with Redis persistence.
//!
//! This module provides a robust background job processing system with:
//! - Type-safe job definitions via the [`Job`] trait
//! - Redis-backed persistence for durability
//! - Automatic retry with exponential backoff
//! - Dead letter queue for failed jobs
//! - Priority-based execution
//! - Graceful shutdown support
//!
//! # Example
//!
//! ```rust
//! use acton_htmx::jobs::{Job, JobContext, JobResult};
//! use async_trait::async_trait;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct WelcomeEmailJob {
//!     user_id: i64,
//!     email: String,
//! }
//!
//! #[async_trait]
//! impl Job for WelcomeEmailJob {
//!     type Result = ();
//!
//!     async fn execute(&self, ctx: &JobContext) -> JobResult<Self::Result> {
//!         // Send welcome email
//!         println!("Sending welcome email to {} (user {})", self.email, self.user_id);
//!         Ok(())
//!     }
//!
//!     fn max_retries(&self) -> u32 {
//!         3
//!     }
//! }
//! ```

mod error;
mod job;
mod status;

pub use error::{JobError, JobResult};
pub use job::{Job, JobId};
pub use status::JobStatus;

// Re-export agent components
pub mod agent;
pub use agent::JobAgent;

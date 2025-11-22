//! Background job processing system using acton-reactive actors.
//!
//! This module provides a robust background job processing system with:
//! - Type-safe job definitions via the [`Job`] trait
//! - **Actor-based architecture** using acton-reactive v5
//! - **In-memory priority queue** with fast synchronous operations (`mutate_on`)
//! - **Concurrent Redis persistence** using async I/O (`act_on`)
//! - Automatic retry with exponential backoff
//! - Dead letter queue for failed jobs
//! - Priority-based execution
//! - Graceful shutdown support
//! - Job scheduling (cron, delayed, recurring)
//! - Comprehensive observability with OpenTelemetry support
//!
//! # Architecture
//!
//! The job system uses acton-reactive's actor model with two handler types:
//!
//! ## `mutate_on` Handlers (Synchronous State Mutations)
//! - **EnqueueJob**: Adds jobs to the in-memory priority queue
//! - Fast, synchronous operations that update agent state
//! - Immediate reply to caller
//!
//! ## `act_on` Handlers (Concurrent Async I/O)
//! - **PersistJob**: Writes job data to Redis (runs concurrently)
//! - **MarkJobCompleted**: Updates job status in Redis
//! - **MarkJobFailed**: Records failures for retry logic
//! - **MoveToDeadLetterQueue**: Archives permanently failed jobs
//! - Non-blocking, can run in parallel with other operations
//!
//! This separation ensures:
//! - **Fast enqueue** (in-memory operation returns immediately)
//! - **Durable persistence** (Redis writes happen asynchronously)
//! - **No blocking** (I/O doesn't block the agent's message processing)
//!
//! # Example
//!
//! ```rust
//! use acton_htmx::jobs::{Job, JobResult};
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
//!     async fn execute(&self) -> JobResult<Self::Result> {
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

mod cancellation;
mod error;
mod job;
mod observability;
mod schedule;
mod status;

pub use cancellation::{
    CancellationToken, JobCancellationManager, JobShutdownCoordinator, ShutdownResult,
};
pub use error::{JobError, JobResult};
pub use job::{Job, JobId};
pub use observability::{JobExecutionContext, JobPerformanceRecorder, JobQueueObserver};
#[cfg(feature = "otel-metrics")]
pub use observability::JobMetricsCollector;
pub use schedule::JobSchedule;
pub use status::JobStatus;

// Re-export agent components
pub mod agent;
pub use agent::JobAgent;

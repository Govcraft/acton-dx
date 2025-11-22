// ! Job cancellation and graceful shutdown support.

use super::JobId;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// A cancellation token for job execution.
///
/// Allows jobs to check if they've been cancelled and respond appropriately.
/// Jobs should periodically call `is_cancelled()` and gracefully terminate if true.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::jobs::CancellationToken;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() {
///     let token = CancellationToken::new();
///
///     // Spawn a task that can be cancelled
///     let token_clone = token.clone();
///     let handle = tokio::spawn(async move {
///         loop {
///             if token_clone.is_cancelled() {
///                 println!("Task cancelled gracefully");
///                 break;
///             }
///
///             // Do work...
///             tokio::time::sleep(Duration::from_millis(100)).await;
///         }
///     });
///
///     // Cancel the task
///     token.cancel();
///
///     // Wait for graceful shutdown
///     let _ = handle.await;
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CancellationToken {
    /// Shared cancellation state.
    state: Arc<CancellationState>,
}

#[derive(Debug)]
struct CancellationState {
    /// Watch channel sender for cancellation signal.
    tx: watch::Sender<bool>,
    /// Watch channel receiver for cancellation signal.
    rx: watch::Receiver<bool>,
}

impl CancellationToken {
    /// Create a new cancellation token.
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            state: Arc::new(CancellationState { tx, rx }),
        }
    }

    /// Check if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.state.rx.borrow()
    }

    /// Request cancellation.
    ///
    /// This will signal all clones of this token that cancellation has been requested.
    pub fn cancel(&self) {
        let _ = self.state.tx.send(true);
        debug!("Cancellation requested");
    }

    /// Wait for cancellation signal.
    ///
    /// Returns immediately if already cancelled.
    pub async fn cancelled(&self) {
        let mut rx = self.state.rx.clone();
        while !*rx.borrow() {
            if rx.changed().await.is_err() {
                // Sender dropped, treat as cancelled
                break;
            }
        }
    }

    /// Run a future with cancellation support.
    ///
    /// If the token is cancelled, the future will be cancelled.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the operation was cancelled.
    pub async fn run_until_cancelled<F, T>(&self, future: F) -> Result<T, ()>
    where
        F: std::future::Future<Output = T>,
    {
        tokio::select! {
            result = future => Ok(result),
            () = self.cancelled() => Err(()),
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Job cancellation coordinator.
///
/// Manages cancellation tokens for running jobs and handles graceful shutdown.
#[derive(Debug, Clone)]
pub struct JobCancellationManager {
    /// Active cancellation tokens indexed by job ID.
    tokens: Arc<RwLock<HashMap<JobId, CancellationToken>>>,
}

impl Default for JobCancellationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobCancellationManager {
    /// Create a new cancellation manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a cancellation token for a job.
    pub fn register(&self, job_id: JobId, token: CancellationToken) {
        self.tokens.write().insert(job_id, token);
        debug!("Registered cancellation token for job {}", job_id);
    }

    /// Unregister a cancellation token (job completed or cancelled).
    pub fn unregister(&self, job_id: &JobId) {
        self.tokens.write().remove(job_id);
        debug!("Unregistered cancellation token for job {}", job_id);
    }

    /// Cancel a specific job.
    ///
    /// Returns `true` if the job was found and cancelled.
    #[must_use] 
    pub fn cancel_job(&self, job_id: &JobId) -> bool {
        self.tokens.read().get(job_id).map_or_else(
            || {
                warn!("Attempted to cancel unknown job {}", job_id);
                false
            },
            |token| {
                token.cancel();
                info!("Cancelled job {}", job_id);
                true
            },
        )
    }

    /// Cancel all running jobs.
    pub fn cancel_all(&self) {
        let tokens = self.tokens.read();
        for (job_id, token) in tokens.iter() {
            token.cancel();
            info!("Cancelled job {}", job_id);
        }
    }

    /// Get the number of jobs with active cancellation tokens.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.tokens.read().len()
    }

    /// Wait for all jobs to complete with a timeout.
    ///
    /// Returns `true` if all jobs completed within the timeout.
    pub async fn wait_for_completion(&self, max_wait: Duration) -> bool {
        let start = std::time::Instant::now();

        loop {
            if self.tokens.read().is_empty() {
                info!("All jobs completed gracefully");
                return true;
            }

            if start.elapsed() >= max_wait {
                let remaining = self.tokens.read().len();
                warn!(
                    "Timeout waiting for job completion: {} jobs still running",
                    remaining
                );
                return false;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// Graceful shutdown coordinator for the job system.
#[derive(Debug, Clone)]
pub struct JobShutdownCoordinator {
    /// Cancellation manager.
    cancellation_manager: JobCancellationManager,
    /// Global shutdown signal.
    shutdown_token: CancellationToken,
}

impl JobShutdownCoordinator {
    /// Create a new shutdown coordinator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancellation_manager: JobCancellationManager::new(),
            shutdown_token: CancellationToken::new(),
        }
    }

    /// Get the cancellation manager.
    #[must_use]
    pub const fn cancellation_manager(&self) -> &JobCancellationManager {
        &self.cancellation_manager
    }

    /// Get the global shutdown token.
    #[must_use]
    pub const fn shutdown_token(&self) -> &CancellationToken {
        &self.shutdown_token
    }

    /// Initiate graceful shutdown.
    ///
    /// This will:
    /// 1. Signal global shutdown (prevent new jobs)
    /// 2. Cancel all running jobs
    /// 3. Wait for jobs to complete (up to timeout)
    /// 4. Force shutdown if timeout exceeded
    ///
    /// # Returns
    ///
    /// `ShutdownResult` indicating whether shutdown was graceful or forced.
    pub async fn shutdown(&self, graceful_timeout: Duration) -> ShutdownResult {
        info!("Initiating job system shutdown");

        // Signal global shutdown
        self.shutdown_token.cancel();

        // Cancel all running jobs
        self.cancellation_manager.cancel_all();

        // Wait for graceful completion
        let graceful = self
            .cancellation_manager
            .wait_for_completion(graceful_timeout)
            .await;

        if graceful {
            info!("Job system shutdown completed gracefully");
            ShutdownResult::Graceful
        } else {
            warn!("Job system forced shutdown after timeout");
            ShutdownResult::Forced {
                jobs_remaining: self.cancellation_manager.active_count(),
            }
        }
    }

    /// Shutdown with a timeout, returning immediately if exceeded.
    ///
    /// # Errors
    ///
    /// Returns `Err` if shutdown times out.
    pub async fn shutdown_with_timeout(
        &self,
        graceful_timeout: Duration,
        total_timeout: Duration,
    ) -> Result<ShutdownResult, ()> {
        timeout(total_timeout, self.shutdown(graceful_timeout))
            .await
            .map_or_else(
                |_| {
                    warn!("Shutdown timeout exceeded");
                    Err(())
                },
                Ok,
            )
    }
}

impl Default for JobShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a shutdown operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShutdownResult {
    /// All jobs completed gracefully within timeout.
    Graceful,
    /// Some jobs did not complete within timeout and were force-stopped.
    Forced {
        /// Number of jobs that were still running.
        jobs_remaining: usize,
    },
}

impl ShutdownResult {
    /// Check if shutdown was graceful.
    #[must_use]
    pub const fn is_graceful(&self) -> bool {
        matches!(self, Self::Graceful)
    }

    /// Get the number of jobs that didn't complete (0 if graceful).
    #[must_use]
    pub const fn jobs_remaining(&self) -> usize {
        match self {
            Self::Graceful => 0,
            Self::Forced { jobs_remaining } => *jobs_remaining,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellation_token_new() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_cancel() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_cancellation_token_clone() {
        let token1 = CancellationToken::new();
        let token2 = token1.clone();

        token1.cancel();
        assert!(token2.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellation_token_cancelled() {
        let token = CancellationToken::new();

        // Cancel in background
        let token_clone = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            token_clone.cancel();
        });

        // Wait for cancellation
        token.cancelled().await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_run_until_cancelled() {
        let token = CancellationToken::new();

        // Task that would run forever
        let token_clone = token.clone();
        let result = tokio::spawn(async move {
            token_clone
                .run_until_cancelled(async {
                    tokio::time::sleep(Duration::from_secs(1000)).await;
                    42
                })
                .await
        });

        // Cancel immediately
        token.cancel();

        // Should return Err(()) because it was cancelled
        let output = result.await.unwrap();
        assert_eq!(output, Err(()));
    }

    #[test]
    fn test_cancellation_manager_register() {
        let manager = JobCancellationManager::new();
        let job_id = JobId::new();
        let token = CancellationToken::new();

        assert_eq!(manager.active_count(), 0);

        manager.register(job_id, token);
        assert_eq!(manager.active_count(), 1);

        manager.unregister(&job_id);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_cancellation_manager_cancel_job() {
        let manager = JobCancellationManager::new();
        let job_id = JobId::new();
        let token = CancellationToken::new();

        manager.register(job_id, token.clone());

        assert!(!token.is_cancelled());
        assert!(manager.cancel_job(&job_id));
        assert!(token.is_cancelled());

        // Cancelling again should still find the job and return true
        // The token remains in the manager until explicitly unregistered
        assert!(manager.cancel_job(&job_id));
    }

    #[test]
    fn test_cancellation_manager_cancel_all() {
        let manager = JobCancellationManager::new();

        let token1 = CancellationToken::new();
        let token2 = CancellationToken::new();

        manager.register(JobId::new(), token1.clone());
        manager.register(JobId::new(), token2.clone());

        manager.cancel_all();

        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
    }

    #[test]
    fn test_shutdown_result() {
        let graceful = ShutdownResult::Graceful;
        assert!(graceful.is_graceful());
        assert_eq!(graceful.jobs_remaining(), 0);

        let forced = ShutdownResult::Forced { jobs_remaining: 5 };
        assert!(!forced.is_graceful());
        assert_eq!(forced.jobs_remaining(), 5);
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_graceful() {
        let coordinator = JobShutdownCoordinator::new();

        // Simulate a job that completes quickly
        let job_id = JobId::new();
        let token = CancellationToken::new();
        coordinator
            .cancellation_manager()
            .register(job_id, token.clone());

        // Spawn task that completes on cancellation
        let token_clone = token.clone();
        tokio::spawn(async move {
            token_clone.cancelled().await;
            // Simulate cleanup
        });

        // Unregister immediately to simulate quick completion
        coordinator.cancellation_manager().unregister(&job_id);

        let result = coordinator.shutdown(Duration::from_secs(1)).await;
        assert!(result.is_graceful());
    }
}

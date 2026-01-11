//! Agent testing utilities for acton-dx
//!
//! This module provides test fixtures and helpers for testing acton-reactive agents
//! with reduced boilerplate. The [`AgentTestRuntime`] wrapper provides automatic
//! cleanup and convenient access to the runtime.
//!
//! # Example
//!
//! ```rust,no_run
//! use acton_htmx::testing::AgentTestRuntime;
//! use acton_htmx::agents::CsrfManagerAgent;
//!
//! #[tokio::test(flavor = "multi_thread")]
//! async fn test_csrf_manager() {
//!     let mut runtime = AgentTestRuntime::new();
//!     let handle = CsrfManagerAgent::spawn(runtime.runtime_mut()).await.unwrap();
//!
//!     // Test logic here...
//!
//!     // Runtime is automatically cleaned up when dropped
//! }
//! ```

use acton_reactive::prelude::ActorRuntime;
use std::time::Duration;
use tokio::sync::oneshot;

/// A test wrapper for acton-reactive runtime with automatic cleanup.
///
/// This wrapper simplifies agent testing by:
/// - Providing a clean runtime for each test
/// - Automatically shutting down the runtime when dropped
/// - Reducing boilerplate in test setup
///
/// # Note on Agent Spawning
///
/// Since acton-reactive agents don't share a common trait for spawning,
/// you spawn agents using their specific `spawn` methods:
///
/// ```rust,no_run
/// use acton_htmx::testing::AgentTestRuntime;
/// use acton_htmx::agents::CsrfManagerAgent;
///
/// # async fn example() {
/// let mut runtime = AgentTestRuntime::new();
/// let handle = CsrfManagerAgent::spawn(runtime.runtime_mut()).await.unwrap();
/// # }
/// ```
///
/// # Example
///
/// ```rust,no_run
/// use acton_htmx::testing::AgentTestRuntime;
/// use acton_htmx::agents::SessionManagerAgent;
///
/// #[tokio::test(flavor = "multi_thread")]
/// async fn test_session_manager() {
///     let mut runtime = AgentTestRuntime::new();
///     let handle = SessionManagerAgent::spawn(runtime.runtime_mut()).await.unwrap();
///
///     // Use the handle...
///
///     // Automatic cleanup on drop - no need to call shutdown manually
/// }
/// ```
pub struct AgentTestRuntime {
    runtime: ActorRuntime,
}

impl AgentTestRuntime {
    /// Create a new test runtime.
    ///
    /// The runtime is automatically shut down when dropped.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use acton_htmx::testing::AgentTestRuntime;
    ///
    /// # async fn example() {
    /// let mut runtime = AgentTestRuntime::new().await;
    /// // Use the runtime...
    /// # }
    /// ```
    pub async fn new() -> Self {
        Self {
            runtime: acton_reactive::prelude::ActonApp::launch_async().await,
        }
    }

    /// Get a mutable reference to the underlying runtime.
    ///
    /// Use this to spawn agents:
    ///
    /// ```rust,no_run
    /// use acton_htmx::testing::AgentTestRuntime;
    /// use acton_htmx::agents::CsrfManagerAgent;
    ///
    /// # async fn example() {
    /// let mut runtime = AgentTestRuntime::new();
    /// let handle = CsrfManagerAgent::spawn(runtime.runtime_mut()).await.unwrap();
    /// # }
    /// ```
    #[must_use]
    pub fn runtime_mut(&mut self) -> &mut ActorRuntime {
        &mut self.runtime
    }

    /// Get a reference to the underlying runtime.
    #[must_use]
    pub const fn runtime(&self) -> &ActorRuntime {
        &self.runtime
    }

    /// Manually shut down the runtime.
    ///
    /// This is called automatically when the runtime is dropped, but you can
    /// call it manually if you need to verify shutdown behavior or ensure
    /// cleanup happens at a specific point.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime fails to shut down properly.
    pub async fn shutdown(mut self) -> Result<(), anyhow::Error> {
        self.runtime.shutdown_all().await
    }
}

// Note: Default is not implemented because new() is async.
// Use AgentTestRuntime::new().await in async tests.

impl Drop for AgentTestRuntime {
    fn drop(&mut self) {
        // Attempt to shutdown gracefully on drop
        // Use try_current to check if we're in a tokio context
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // Take the runtime to own it for the async block
            let mut runtime = std::mem::take(&mut self.runtime);
            // Spawn the shutdown in the background - we can't block in drop
            handle.spawn(async move {
                let _ = runtime.shutdown_all().await;
            });
        }
    }
}

/// Await a response from an agent with a default timeout of 1 second.
///
/// This helper reduces boilerplate for the common pattern of waiting for
/// a oneshot channel response with a timeout.
///
/// # Arguments
///
/// * `rx` - The oneshot receiver to wait on
///
/// # Returns
///
/// The received value.
///
/// # Panics
///
/// Panics if:
/// - The timeout (1 second) is exceeded
/// - The sender is dropped without sending a value
///
/// # Example
///
/// ```rust
/// use tokio::sync::oneshot;
/// use acton_htmx::testing::await_response;
///
/// # #[tokio::main]
/// # async fn main() {
/// let (tx, rx) = oneshot::channel::<String>();
/// tx.send("hello".to_string()).unwrap();
///
/// let value = await_response(rx).await;
/// assert_eq!(value, "hello");
/// # }
/// ```
pub async fn await_response<T>(rx: oneshot::Receiver<T>) -> T {
    await_response_with_timeout(rx, Duration::from_secs(1)).await
}

/// Await a response from an agent with a custom timeout.
///
/// # Arguments
///
/// * `rx` - The oneshot receiver to wait on
/// * `timeout` - The maximum time to wait for the response
///
/// # Returns
///
/// The received value.
///
/// # Panics
///
/// Panics if:
/// - The timeout is exceeded
/// - The sender is dropped without sending a value
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use tokio::sync::oneshot;
/// use acton_htmx::testing::await_response_with_timeout;
///
/// # #[tokio::main]
/// # async fn main() {
/// let (tx, rx) = oneshot::channel::<String>();
/// tx.send("hello".to_string()).unwrap();
///
/// let value = await_response_with_timeout(rx, Duration::from_secs(5)).await;
/// assert_eq!(value, "hello");
/// # }
/// ```
pub async fn await_response_with_timeout<T>(rx: oneshot::Receiver<T>, timeout: Duration) -> T {
    tokio::time::timeout(timeout, rx)
        .await
        .expect("Timeout waiting for agent response")
        .expect("Agent response channel closed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_agent_test_runtime_creation() {
        let runtime = AgentTestRuntime::new().await;
        drop(runtime); // Should not panic
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_await_response_success() {
        let (tx, rx) = oneshot::channel::<String>();
        tx.send("test".to_string()).unwrap();

        let result = await_response(rx).await;
        assert_eq!(result, "test");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_await_response_with_custom_timeout() {
        let (tx, rx) = oneshot::channel::<i32>();
        tx.send(42).unwrap();

        let result = await_response_with_timeout(rx, Duration::from_millis(100)).await;
        assert_eq!(result, 42);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_runtime_mut_returns_mutable_ref() {
        let mut runtime = AgentTestRuntime::new().await;
        let _runtime_ref = runtime.runtime_mut();
        // If this compiles, we got a mutable reference
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_runtime_returns_ref() {
        let runtime = AgentTestRuntime::new().await;
        let _runtime_ref = runtime.runtime();
        // If this compiles, we got a reference
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_explicit_shutdown() {
        let runtime = AgentTestRuntime::new().await;
        let result = runtime.shutdown().await;
        assert!(result.is_ok());
    }
}

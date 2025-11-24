//! Request-reply pattern helpers for web handler agents
//!
//! This module provides utilities for implementing the request-reply pattern
//! in acton-reactive agents that need to respond synchronously to web handler
//! requests. This pattern is commonly used when Axum extractors need to wait
//! for agent responses.
//!
//! # Why This Pattern?
//!
//! Axum extractors run synchronously (from the handler's perspective) but need
//! to communicate with asynchronous agents. The oneshot channel provides a way
//! to:
//! 1. Send a request to an agent
//! 2. Await the agent's response
//! 3. Return the result to the extractor
//!
//! The `Arc<Mutex<Option<...>>>` wrapping is required because:
//! - `Arc`: Messages must be cloneable for agent supervision/retry
//! - `Mutex`: Interior mutability to take the sender when responding
//! - `Option`: Allows taking ownership of the sender exactly once
//!
//! # Example Usage
//!
//! ```rust
//! use acton_reactive::prelude::*;
//! use crate::agents::request_reply::{create_request_reply, send_response, ResponseChannel};
//! use crate::auth::session::SessionId;
//!
//! #[derive(Clone, Debug)]
//! pub struct GetSessionRequest {
//!     pub session_id: SessionId,
//!     pub response_tx: ResponseChannel<Option<SessionData>>,
//! }
//!
//! impl GetSessionRequest {
//!     pub fn new(session_id: SessionId) -> (Self, oneshot::Receiver<Option<SessionData>>) {
//!         let (response_tx, rx) = create_request_reply();
//!         let request = Self { session_id, response_tx };
//!         (request, rx)
//!     }
//! }
//!
//! // In the agent's message handler:
//! async fn handle_get_session(request: GetSessionRequest) {
//!     let session = load_session(&request.session_id);
//!     let _ = send_response(request.response_tx, session).await;
//! }
//! ```

use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

/// Standard response channel type for web handler requests
///
/// This type wraps a oneshot sender in `Arc<Mutex<Option<...>>>` to satisfy
/// the requirements of acton-reactive message passing:
/// - `Arc`: Messages must be `Clone` for agent supervision/retry
/// - `Mutex`: Provides interior mutability to take ownership of the sender
/// - `Option`: Allows taking the sender exactly once when responding
///
/// # Type Parameter
///
/// * `T` - The type of value to send through the channel
pub type ResponseChannel<T> = Arc<Mutex<Option<oneshot::Sender<T>>>>;

/// Create a request-reply pair with proper channel wrapping
///
/// This is a convenience function that creates both sides of a request-reply
/// communication pattern. The sender is wrapped in `Arc<Mutex<Option<...>>>`
/// for use in agent messages, while the receiver is returned directly for
/// awaiting the response.
///
/// # Returns
///
/// A tuple of `(ResponseChannel<T>, oneshot::Receiver<T>)` where:
/// - The `ResponseChannel` should be included in your request message
/// - The `Receiver` should be awaited by the web handler
///
/// # Example
///
/// ```rust
/// use crate::agents::request_reply::create_request_reply;
///
/// let (response_tx, rx) = create_request_reply();
/// let request = MyRequest {
///     data: "foo".to_string(),
///     response_tx,
/// };
///
/// // Send request to agent
/// agent_handle.send(request).await;
///
/// // Wait for response
/// let response = rx.await.expect("Agent dropped response channel");
/// ```
#[must_use]
pub fn create_request_reply<T>() -> (ResponseChannel<T>, oneshot::Receiver<T>) {
    let (tx, rx) = oneshot::channel();
    (Arc::new(Mutex::new(Some(tx))), rx)
}

/// Send a response through a response channel
///
/// This function properly unwraps and uses the response channel to send a value
/// back to the waiting web handler. The channel is consumed after sending.
///
/// # Parameters
///
/// * `response_tx` - The response channel from the request
/// * `value` - The value to send back to the requester
///
/// # Returns
///
/// `Ok(())` if the response was successfully sent, or `Err(value)` if the
/// receiver was dropped (which typically means the web handler timed out or
/// the client disconnected).
///
/// # Errors
///
/// Returns `Err(value)` if:
/// - The receiver was dropped (client disconnected or handler timed out)
/// - The channel was already used
///
/// # Example
///
/// ```rust
/// use crate::agents::request_reply::send_response;
///
/// async fn handle_request(request: MyRequest) {
///     let result = perform_operation();
///
///     // Send response back to web handler
///     if send_response(request.response_tx, result).await.is_err() {
///         tracing::warn!("Client disconnected before receiving response");
///     }
/// }
/// ```
pub async fn send_response<T>(response_tx: ResponseChannel<T>, value: T) -> Result<(), T> {
    // Take the sender from the Arc<Mutex<Option<...>>>
    // Avoid holding the lock across the send operation
    let tx = response_tx.lock().await.take();
    if let Some(tx) = tx {
        // Send the value through the oneshot channel
        tx.send(value)
    } else {
        // Channel was already used or dropped
        Err(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_request_reply() {
        let (response_tx, rx) = create_request_reply::<String>();

        // Simulate sending response from agent
        let result = send_response(response_tx, "test response".to_string()).await;
        assert!(result.is_ok());

        // Simulate receiving in web handler
        let response = rx.await.expect("Should receive response");
        assert_eq!(response, "test response");
    }

    #[tokio::test]
    async fn test_send_response_with_dropped_receiver() {
        let (response_tx, rx) = create_request_reply::<String>();

        // Drop the receiver (simulating client disconnect)
        drop(rx);

        // Try to send - should fail
        let result = send_response(response_tx, "test response".to_string()).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "test response");
    }

    #[tokio::test]
    async fn test_send_response_twice_fails() {
        let (response_tx, rx) = create_request_reply::<String>();

        // First send should succeed
        let response_tx_clone = response_tx.clone();
        let result1 = send_response(response_tx, "first".to_string()).await;
        assert!(result1.is_ok());

        // Second send with cloned channel should fail (sender already taken)
        let result2 = send_response(response_tx_clone, "second".to_string()).await;
        assert!(result2.is_err());

        // Receiver should get the first value
        let response = rx.await.expect("Should receive response");
        assert_eq!(response, "first");
    }
}

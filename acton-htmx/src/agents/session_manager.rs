//! Session Manager Agent
//!
//! Actor-based session management using acton-reactive.
//! Implements hybrid in-memory + Redis storage strategy.
//!
//! This module uses unified message patterns that support both:
//! 1. **Agent-to-Agent**: Using `reply_envelope` for inter-agent communication
//! 2. **Web Handler**: Using optional oneshot channels for request-reply from Axum handlers
//!
//! Messages with optional `response_tx` fields can be used from both contexts.

use crate::agents::request_reply::{create_request_reply, send_response, ResponseChannel};
use crate::agents::default_agent_config;
use crate::auth::session::{FlashMessage, SessionData, SessionId};
use acton_reactive::prelude::*;
use chrono::{DateTime, Duration, Utc};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use tokio::sync::oneshot;

// Type alias for the ManagedAgent builder type
type SessionAgentBuilder = ManagedAgent<Idle, SessionManagerAgent>;

#[cfg(feature = "redis")]
use deadpool_redis::Pool as RedisPool;

/// Session manager agent model
#[derive(Debug, Default, Clone)]
pub struct SessionManagerAgent {
    /// In-memory session storage
    sessions: HashMap<SessionId, SessionData>,
    /// Expiry queue for cleanup (min-heap by expiration time)
    expiry_queue: BinaryHeap<Reverse<(DateTime<Utc>, SessionId)>>,
    /// Optional Redis backend for distributed sessions
    #[cfg(feature = "redis")]
    redis: Option<RedisPool>,
}

// ============================================================================
// Unified Messages (support both web handlers and agent-to-agent)
// ============================================================================

/// Load a session by ID
///
/// Supports both web handler (with response_tx) and agent-to-agent (reply_envelope) patterns.
#[derive(Clone, Debug)]
pub struct LoadSession {
    /// The session ID to load
    pub session_id: SessionId,
    /// Optional response channel for web handlers
    pub response_tx: Option<ResponseChannel<Option<SessionData>>>,
}

impl LoadSession {
    /// Create a new load session message for agent-to-agent communication
    #[must_use]
    pub const fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            response_tx: None,
        }
    }

    /// Create a new load session request with response channel for web handlers
    #[must_use]
    pub fn with_response(session_id: SessionId) -> (Self, oneshot::Receiver<Option<SessionData>>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Save session data
///
/// Supports both web handler (with response_tx) and agent-to-agent patterns.
#[derive(Clone, Debug)]
pub struct SaveSession {
    /// The session ID to save
    pub session_id: SessionId,
    /// The session data to persist
    pub data: SessionData,
    /// Optional response channel for confirmation
    pub response_tx: Option<ResponseChannel<bool>>,
}

impl SaveSession {
    /// Create a new save session message (fire-and-forget)
    #[must_use]
    pub const fn new(session_id: SessionId, data: SessionData) -> Self {
        Self {
            session_id,
            data,
            response_tx: None,
        }
    }

    /// Create a new save session request with confirmation
    #[must_use]
    pub fn with_confirmation(
        session_id: SessionId,
        data: SessionData,
    ) -> (Self, oneshot::Receiver<bool>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            data,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Get and clear flash messages from a session
///
/// Supports both web handler (with response_tx) and agent-to-agent (reply_envelope) patterns.
#[derive(Clone, Debug)]
pub struct TakeFlashes {
    /// The session ID to retrieve flashes from
    pub session_id: SessionId,
    /// Optional response channel for web handlers
    pub response_tx: Option<ResponseChannel<Vec<FlashMessage>>>,
}

impl TakeFlashes {
    /// Create a new take flashes message for agent-to-agent communication
    #[must_use]
    pub const fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            response_tx: None,
        }
    }

    /// Create a new take flashes request with response channel for web handlers
    #[must_use]
    pub fn with_response(session_id: SessionId) -> (Self, oneshot::Receiver<Vec<FlashMessage>>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Message to delete a session by ID
#[derive(Clone, Debug)]
pub struct DeleteSession {
    /// The session ID to delete
    pub session_id: SessionId,
}

/// Message to trigger cleanup of expired sessions
#[derive(Clone, Debug)]
pub struct CleanupExpired;

/// Message to add a flash message to a session
#[derive(Clone, Debug)]
pub struct AddFlash {
    /// The session ID to add the flash to
    pub session_id: SessionId,
    /// The flash message to add
    pub message: FlashMessage,
}

impl SessionManagerAgent {
    /// Spawn session manager agent without Redis backend
    ///
    /// Uses in-memory storage only. Suitable for development or single-instance deployments.
    ///
    /// # Errors
    ///
    /// Returns error if agent initialization fails
    pub async fn spawn(runtime: &mut AgentRuntime) -> anyhow::Result<AgentHandle> {
        let config = default_agent_config("session_manager")?;
        let builder = runtime.new_agent_with_config::<Self>(config).await;
        Self::configure_handlers(builder).await
    }

    /// Spawn session manager with Redis backend
    ///
    /// Uses Redis for distributed session storage with in-memory caching.
    ///
    /// # Errors
    ///
    /// Returns error if agent initialization fails
    #[cfg(feature = "redis")]
    pub async fn spawn_with_redis(
        runtime: &mut AgentRuntime,
        redis_pool: RedisPool,
    ) -> anyhow::Result<AgentHandle> {
        let config = default_agent_config("session_manager")?;
        let mut builder = runtime.new_agent_with_config::<Self>(config).await;
        builder.model.redis = Some(redis_pool);
        Self::configure_handlers(builder).await
    }

    /// Configure all message handlers for the session manager
    async fn configure_handlers(mut builder: SessionAgentBuilder) -> anyhow::Result<AgentHandle> {
        builder
            // ================================================================
            // Unified Handlers (support both web and agent-to-agent patterns)
            // ================================================================
            .act_on::<LoadSession>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let response_tx = envelope.message().response_tx.clone();
                let session = agent.model.sessions.get(&session_id).cloned();
                let reply_envelope = envelope.reply_envelope();

                Box::pin(async move {
                    // Use validate_and_touch to combine expiry check and touch
                    let result = session.and_then(|mut data| {
                        if data.validate_and_touch(Duration::hours(24)) {
                            Some(data)
                        } else {
                            None
                        }
                    });

                    // Send response to web handler if channel provided
                    if let Some(tx) = response_tx {
                        let _ = send_response(tx, result.clone()).await;
                    }

                    // Always send reply envelope for agent-to-agent
                    let _: () = reply_envelope.send(result).await;
                })
            })
            .mutate_on::<SaveSession>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let data = envelope.message().data.clone();
                let response_tx = envelope.message().response_tx.clone();

                agent
                    .model
                    .sessions
                    .insert(session_id.clone(), data.clone());
                agent
                    .model
                    .expiry_queue
                    .push(Reverse((data.expires_at, session_id)));

                AgentReply::from_async(async move {
                    // Send confirmation to web handler if channel provided
                    if let Some(tx) = response_tx {
                        let _ = send_response(tx, true).await;
                    }
                })
            })
            .mutate_on::<TakeFlashes>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let response_tx = envelope.message().response_tx.clone();
                let reply_envelope = envelope.reply_envelope();

                // Take and clear flash messages atomically
                let messages = agent
                    .model
                    .sessions
                    .get_mut(&session_id)
                    .map(|session| std::mem::take(&mut session.flash_messages))
                    .unwrap_or_default();

                AgentReply::from_async(async move {
                    // Send response to web handler if channel provided
                    if let Some(tx) = response_tx {
                        let _ = send_response(tx, messages.clone()).await;
                    }

                    // Always send reply envelope for agent-to-agent
                    let _: () = reply_envelope.send(messages).await;
                })
            })
            .mutate_on::<DeleteSession>(|agent, envelope| {
                agent.model.sessions.remove(&envelope.message().session_id);
                AgentReply::immediate()
            })
            .mutate_on::<CleanupExpired>(|agent, _envelope| {
                let now = Utc::now();
                let mut expired = Vec::new();

                loop {
                    let should_pop = agent
                        .model
                        .expiry_queue
                        .peek()
                        .is_some_and(|Reverse((expiry, _))| *expiry <= now);

                    if should_pop {
                        if let Some(Reverse((_, session_id))) = agent.model.expiry_queue.pop() {
                            expired.push(session_id);
                        }
                    } else {
                        break;
                    }
                }

                for session_id in expired {
                    agent.model.sessions.remove(&session_id);
                }

                AgentReply::immediate()
            })
            .mutate_on::<AddFlash>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let message = envelope.message().message.clone();

                if let Some(session) = agent.model.sessions.get_mut(&session_id) {
                    session.flash_messages.push(message);
                }

                AgentReply::immediate()
            });

        Ok(builder.start().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_manager_creation() {
        let mut runtime = ActonApp::launch();
        let result = SessionManagerAgent::spawn(&mut runtime).await;
        assert!(result.is_ok());
        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_save_and_load_with_verification() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let mut data = SessionData::new();
        data.set("test_key".to_string(), "test_value".to_string())
            .unwrap();

        // Save session
        session_manager
            .send(SaveSession::new(session_id.clone(), data.clone()))
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Load session using web handler style (with oneshot channel for verification)
        let (request, rx) = LoadSession::with_response(session_id.clone());
        session_manager.send(request).await;

        // Verify response
        let loaded_data = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout waiting for response")
            .expect("Channel closed");

        assert!(loaded_data.is_some(), "Session should exist");
        let loaded = loaded_data.unwrap();
        let loaded_value: Option<String> = loaded.get("test_key").unwrap();
        assert_eq!(loaded_value, Some("test_value".to_string()));

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_not_found() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();

        // Try to load non-existent session
        let (request, rx) = LoadSession::with_response(session_id);
        session_manager.send(request).await;

        // Verify response
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout waiting for response")
            .expect("Channel closed");

        assert!(result.is_none(), "Session should not exist");

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_delete_with_verification() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let data = SessionData::new();

        // Save session
        session_manager
            .send(SaveSession::new(session_id.clone(), data))
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify session exists
        let (request, rx) = LoadSession::with_response(session_id.clone());
        session_manager.send(request).await;
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");
        assert!(result.is_some(), "Session should exist before deletion");

        // Delete session
        session_manager
            .send(DeleteSession {
                session_id: session_id.clone(),
            })
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify session is deleted
        let (request, rx) = LoadSession::with_response(session_id);
        session_manager.send(request).await;
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");
        assert!(result.is_none(), "Session should not exist after deletion");

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_flash_messages_with_verification() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let data = SessionData::new();

        // Save session first
        session_manager
            .send(SaveSession::new(session_id.clone(), data))
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Add flash messages
        session_manager
            .send(AddFlash {
                session_id: session_id.clone(),
                message: FlashMessage::success("Success message"),
            })
            .await;

        session_manager
            .send(AddFlash {
                session_id: session_id.clone(),
                message: FlashMessage::error("Error message"),
            })
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Get and verify flashes (using TakeFlashesRequest which clears them)
        let (request, rx) = TakeFlashes::with_response(session_id.clone());
        session_manager.send(request).await;

        let flashes = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout waiting for response")
            .expect("Channel closed");

        assert_eq!(flashes.len(), 2, "Should have 2 flash messages");
        assert_eq!(flashes[0].message, "Success message");
        assert_eq!(flashes[1].message, "Error message");

        // Verify flashes are cleared after taking
        let (request, rx) = TakeFlashes::with_response(session_id);
        session_manager.send(request).await;

        let flashes = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert_eq!(flashes.len(), 0, "Flashes should be cleared after taking");

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_expiry_cleanup() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let mut data = SessionData::new();
        // Set expiry to the past
        data.expires_at = Utc::now() - Duration::hours(1);

        // Save expired session
        session_manager
            .send(SaveSession::new(session_id.clone(), data))
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Trigger cleanup
        session_manager.send(CleanupExpired).await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Verify expired session is not returned
        let (request, rx) = LoadSession::with_response(session_id);
        session_manager.send(request).await;

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert!(result.is_none(), "Expired session should not be returned");

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_touch_extends_expiry() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let mut data = SessionData::new();
        let original_expiry = Utc::now() + Duration::hours(1);
        data.expires_at = original_expiry;

        // Save session
        session_manager
            .send(SaveSession::new(session_id.clone(), data))
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Load session (which should touch and extend expiry)
        let (request, rx) = LoadSession::with_response(session_id);
        session_manager.send(request).await;

        let loaded = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert!(loaded.is_some(), "Session should exist");
        let loaded_data = loaded.unwrap();
        assert!(
            loaded_data.expires_at > original_expiry,
            "Expiry should be extended after touch"
        );

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_save_with_confirmation() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let data = SessionData::new();

        // Save with confirmation
        let (request, rx) = SaveSession::with_confirmation(session_id, data);
        session_manager.send(request).await;

        // Verify confirmation
        let confirmed = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout waiting for confirmation")
            .expect("Channel closed");

        assert!(confirmed, "Save should be confirmed");

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_flash_messages() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let data = SessionData::new();

        // Save session
        session_manager
            .send(SaveSession::new(session_id.clone(), data))
            .await;

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Add multiple flash messages concurrently
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let sm = session_manager.clone();
                let sid = session_id.clone();
                tokio::spawn(async move {
                    sm.send(AddFlash {
                        session_id: sid,
                        message: FlashMessage::info(format!("Message {i}")),
                    })
                    .await;
                })
            })
            .collect();

        // Wait for all sends to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Allow message processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Retrieve flashes
        let (request, rx) = TakeFlashes::with_response(session_id);
        session_manager.send(request).await;

        let flashes = tokio::time::timeout(tokio::time::Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert_eq!(
            flashes.len(),
            10,
            "Should have all 10 flash messages despite concurrent adds"
        );

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }
}

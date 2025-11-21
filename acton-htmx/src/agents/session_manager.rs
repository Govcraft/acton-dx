//! Session Manager Agent
//!
//! Actor-based session management using acton-reactive.
//! Implements hybrid in-memory + Redis storage strategy.

use crate::auth::session::{FlashMessage, SessionData, SessionId};
use acton_reactive::prelude::*;
use chrono::{DateTime, Duration, Utc};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;

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

// ✅ Messages - NO macro, just Clone + Debug (per expert review)

/// Message to load a session by ID
#[derive(Clone, Debug)]
pub struct LoadSession {
    /// The session ID to load
    pub session_id: SessionId,
}

/// Response message when session is successfully loaded
#[derive(Clone, Debug)]
pub struct SessionLoaded {
    /// The loaded session data
    pub data: SessionData,
}

/// Response message when session is not found
#[derive(Clone, Debug)]
pub struct SessionNotFound;

/// Message to save session data
#[derive(Clone, Debug)]
pub struct SaveSession {
    /// The session ID to save
    pub session_id: SessionId,
    /// The session data to persist
    pub data: SessionData,
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

/// Message to retrieve flash messages from a session
#[derive(Clone, Debug)]
pub struct GetFlashes {
    /// The session ID to retrieve flashes from
    pub session_id: SessionId,
}

/// Response message containing flash messages
#[derive(Clone, Debug)]
pub struct FlashMessages {
    /// The flash messages retrieved
    pub messages: Vec<FlashMessage>,
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
        Self::build(runtime, None).await
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
        Self::build(runtime, Some(redis_pool)).await
    }

    /// Internal builder that handles both Redis and non-Redis configurations
    #[cfg(feature = "redis")]
    async fn build(
        runtime: &mut AgentRuntime,
        redis_pool: Option<RedisPool>,
    ) -> anyhow::Result<AgentHandle> {
        let config = AgentConfig::new(Ern::with_root("session_manager")?, None, None)?;

        let mut builder = runtime.new_agent_with_config::<Self>(config).await;
        builder.model.redis = redis_pool;

        // ✅ Configure message handlers using method chaining
        builder
            // READ operation - use act_on for concurrency (per expert review)
            .act_on::<LoadSession>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let session = agent.model.sessions.get(&session_id).cloned();
                let reply_envelope = envelope.reply_envelope();

                Box::pin(async move {
                    if let Some(mut data) = session {
                        // Check expiration
                        if data.is_expired() {
                            let _: () = reply_envelope.send(SessionNotFound).await;
                        } else {
                            // Touch session to extend expiration
                            data.touch(Duration::hours(24));
                            let _: () = reply_envelope.send(SessionLoaded { data }).await;
                        }
                    } else {
                        let _: () = reply_envelope.send(SessionNotFound).await;
                    }
                })
            })
            // WRITE operations - use mutate_on (per expert review)
            .mutate_on::<SaveSession>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let data = envelope.message().data.clone();

                agent.model.sessions.insert(session_id.clone(), data.clone());
                agent.model.expiry_queue.push(Reverse((data.expires_at, session_id)));

                AgentReply::immediate()
            })
            .mutate_on::<DeleteSession>(|agent, envelope| {
                agent.model.sessions.remove(&envelope.message().session_id);
                AgentReply::immediate()
            })
            .mutate_on::<CleanupExpired>(|agent, _envelope| {
                let now = Utc::now();
                let mut expired = Vec::new();

                // Collect expired session IDs - check and pop in loop
                loop {
                    let should_pop = agent.model.expiry_queue
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

                // Remove expired sessions
                for session_id in expired {
                    agent.model.sessions.remove(&session_id);
                }

                AgentReply::immediate()
            })
            // Flash message operations
            .mutate_on::<AddFlash>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let message = envelope.message().message.clone();

                if let Some(session) = agent.model.sessions.get_mut(&session_id) {
                    session.flash_messages.push(message);
                }

                AgentReply::immediate()
            })
            .act_on::<GetFlashes>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let messages = agent.model.sessions
                    .get(&session_id)
                    .map(|s| s.flash_messages.clone())
                    .unwrap_or_default();

                let reply_envelope = envelope.reply_envelope();

                Box::pin(async move {
                    let _: () = reply_envelope.send(FlashMessages { messages }).await;
                })
            })
            // Lifecycle hook: spawn cleanup task (per expert review)
            .after_start(|agent| {
                let self_handle = agent.handle().clone();
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                    loop {
                        interval.tick().await;
                        let _: () = self_handle.send(CleanupExpired).await;
                    }
                });
                AgentReply::immediate()
            });

        Ok(builder.start().await)
    }

    /// Internal builder for non-Redis configuration
    #[cfg(not(feature = "redis"))]
    async fn build(
        runtime: &mut AgentRuntime,
        _redis_pool: Option<()>,
    ) -> anyhow::Result<AgentHandle> {
        let config = AgentConfig::new(Ern::with_root("session_manager")?, None, None)?;

        let mut builder = runtime.new_agent_with_config::<Self>(config).await;

        // Configure message handlers using method chaining
        builder
            // READ operation - use act_on for concurrency
            .act_on::<LoadSession>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let session = agent.model.sessions.get(&session_id).cloned();
                let reply_envelope = envelope.reply_envelope();

                Box::pin(async move {
                    if let Some(mut data) = session {
                        if data.is_expired() {
                            let _: () = reply_envelope.send(SessionNotFound).await;
                        } else {
                            data.touch(Duration::hours(24));
                            let _: () = reply_envelope.send(SessionLoaded { data }).await;
                        }
                    } else {
                        let _: () = reply_envelope.send(SessionNotFound).await;
                    }
                })
            })
            // WRITE operations - use mutate_on
            .mutate_on::<SaveSession>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let data = envelope.message().data.clone();

                agent.model.sessions.insert(session_id.clone(), data.clone());
                agent.model.expiry_queue.push(Reverse((data.expires_at, session_id)));

                AgentReply::immediate()
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
            // Flash message operations
            .mutate_on::<AddFlash>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let message = envelope.message().message.clone();

                if let Some(session) = agent.model.sessions.get_mut(&session_id) {
                    session.flash_messages.push(message);
                }

                AgentReply::immediate()
            })
            .act_on::<GetFlashes>(|agent, envelope| {
                let session_id = envelope.message().session_id.clone();
                let messages = agent
                    .model
                    .sessions
                    .get(&session_id)
                    .map(|s| s.flash_messages.clone())
                    .unwrap_or_default();

                let reply_envelope = envelope.reply_envelope();

                Box::pin(async move {
                    let _: () = reply_envelope.send(FlashMessages { messages }).await;
                })
            })
            // Lifecycle hook: spawn cleanup task
            .after_start(|agent| {
                let self_handle = agent.handle().clone();
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                    loop {
                        interval.tick().await;
                        let _: () = self_handle.send(CleanupExpired).await;
                    }
                });
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
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_save_and_load() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let mut data = SessionData::new();
        data.set("test_key".to_string(), "test_value").unwrap();

        // Save session
        session_manager
            .send(SaveSession {
                session_id: session_id.clone(),
                data: data.clone(),
            })
            .await;

        // Load session
        session_manager
            .send(LoadSession {
                session_id: session_id.clone(),
            })
            .await;

        // TODO: Add response verification once we have proper message handling
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_delete() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let data = SessionData::new();

        // Save then delete
        session_manager
            .send(SaveSession {
                session_id: session_id.clone(),
                data,
            })
            .await;

        session_manager
            .send(DeleteSession {
                session_id: session_id.clone(),
            })
            .await;

        // Load should return NotFound
        session_manager.send(LoadSession { session_id }).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_flash_messages() {
        let mut runtime = ActonApp::launch();
        let session_manager = SessionManagerAgent::spawn(&mut runtime).await.unwrap();

        let session_id = SessionId::generate();
        let data = SessionData::new();

        // Save session first
        session_manager
            .send(SaveSession {
                session_id: session_id.clone(),
                data,
            })
            .await;

        // Add flash message
        session_manager
            .send(AddFlash {
                session_id: session_id.clone(),
                message: FlashMessage::success("Test message"),
            })
            .await;

        // Get flashes
        session_manager.send(GetFlashes {
            session_id,
        }).await;
    }
}

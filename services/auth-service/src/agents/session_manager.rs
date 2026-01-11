//! Session Manager Agent for auth-service.
//!
//! Uses acton-reactive for concurrent session state management with
//! proper isolation between reads and writes.

use crate::{FlashMessage, SessionData};
use acton_reactive::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};

/// Type alias for response channels (cloneable for actor message requirements).
pub type ResponseChannel<T> = Arc<Mutex<Option<oneshot::Sender<T>>>>;

/// Create a request-reply pair.
#[must_use]
pub fn create_request_reply<T>() -> (ResponseChannel<T>, oneshot::Receiver<T>) {
    let (tx, rx) = oneshot::channel();
    (Arc::new(Mutex::new(Some(tx))), rx)
}

/// Send a response through a response channel.
///
/// # Errors
///
/// Returns the value if the receiver was dropped.
pub async fn send_response<T>(response_tx: ResponseChannel<T>, value: T) -> Result<(), T> {
    let tx = response_tx.lock().await.take();
    if let Some(tx) = tx {
        tx.send(value)
    } else {
        Err(value)
    }
}

/// Session manager agent state.
#[derive(Debug, Default)]
pub struct SessionManagerAgent {
    /// In-memory session storage.
    sessions: HashMap<String, SessionData>,
    /// Cleanup interval in seconds.
    cleanup_interval_secs: u64,
}

impl SessionManagerAgent {
    /// Create a new session manager with the given cleanup interval.
    #[must_use]
    pub fn new(cleanup_interval_secs: u64) -> Self {
        Self {
            sessions: HashMap::new(),
            cleanup_interval_secs,
        }
    }

    /// Spawn the session manager agent.
    ///
    /// # Errors
    ///
    /// Returns error if agent initialization fails.
    ///
    /// # Panics
    ///
    /// Panics if the ERN "auth-service" is invalid (which should not happen).
    pub async fn spawn(
        runtime: &mut ActorRuntime,
        cleanup_interval_secs: u64,
    ) -> anyhow::Result<ActorHandle> {
        let config = ActorConfig::new(
            Ern::with_root("auth-service").expect("auth-service is a valid ERN"),
            None,
            None,
        )?;
        let mut builder = runtime.new_actor_with_config::<Self>(config);
        builder.model = Self::new(cleanup_interval_secs);
        let cleanup_interval = builder.model.cleanup_interval_secs;

        Self::configure_handlers(&mut builder);

        let handle = builder.start().await;
        Self::spawn_cleanup_task(handle.clone(), cleanup_interval);
        Ok(handle)
    }

    /// Configure all message handlers using inline closures that delegate to logic helpers.
    fn configure_handlers(builder: &mut ManagedActor<Idle, Self>) {
        builder
            .mutate_on::<CreateSession>(|agent, ctx| {
                let msg = ctx.message();
                let session = SessionData::new(msg.ttl_seconds, msg.user_id);
                let response_session = session.clone();
                let response_tx = msg.response_tx.clone();
                agent.model.sessions.insert(session.session_id.clone(), session);
                Reply::pending(send_optional_response(response_tx, response_session))
            })
            .act_on::<LoadSession>(|agent, ctx| {
                let msg = ctx.message();
                let session = agent.model.sessions.get(&msg.session_id).cloned();
                let response_tx = msg.response_tx.clone();
                Reply::pending(send_optional_response(response_tx, session))
            })
            .mutate_on::<UpdateSession>(|agent, ctx| {
                let msg = ctx.message();
                let result = update_session_data(&mut agent.model.sessions, msg);
                let response_tx = msg.response_tx.clone();
                Reply::pending(send_optional_response(response_tx, result))
            })
            .mutate_on::<DeleteSession>(|agent, ctx| {
                let msg = ctx.message();
                let deleted = agent.model.sessions.remove(&msg.session_id).is_some();
                let response_tx = msg.response_tx.clone();
                Reply::pending(send_optional_response(response_tx, deleted))
            })
            .mutate_on::<AddFlash>(|agent, ctx| {
                let msg = ctx.message();
                let success = add_flash_to_session(&mut agent.model.sessions, msg);
                let response_tx = msg.response_tx.clone();
                Reply::pending(send_optional_response(response_tx, success))
            })
            .mutate_on::<TakeFlashes>(|agent, ctx| {
                let msg = ctx.message();
                let flashes = take_flashes_from_session(&mut agent.model.sessions, &msg.session_id);
                let response_tx = msg.response_tx.clone();
                Reply::pending(send_optional_response(response_tx, flashes))
            })
            .mutate_on::<CleanupExpired>(|agent, _ctx| {
                agent.model.sessions.retain(|_, session| !session.is_expired());
                tracing::debug!("Cleaned up sessions, remaining: {}", agent.model.sessions.len());
                Reply::ready()
            });
    }

    /// Spawn the periodic cleanup background task.
    fn spawn_cleanup_task(handle: ActorHandle, interval_secs: u64) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                handle.send(CleanupExpired).await;
            }
        });
    }
}

// ============================================================================
// Logic Helper Functions (extracted to reduce handler closure line counts)
// ============================================================================

/// Send a response if a response channel is provided.
async fn send_optional_response<T>(response_tx: Option<ResponseChannel<T>>, value: T) {
    if let Some(tx) = response_tx {
        let _ = send_response(tx, value).await;
    }
}

/// Update session data and return the updated session.
fn update_session_data(
    sessions: &mut HashMap<String, SessionData>,
    msg: &UpdateSession,
) -> Option<SessionData> {
    sessions.get_mut(&msg.session_id).map(|session| {
        for (key, value) in &msg.data {
            session.data.insert(key.clone(), value.clone());
        }
        if let Some(uid) = msg.user_id {
            session.user_id = Some(uid);
        }
        session.clone()
    })
}

/// Add a flash message to a session.
fn add_flash_to_session(
    sessions: &mut HashMap<String, SessionData>,
    msg: &AddFlash,
) -> bool {
    sessions.get_mut(&msg.session_id).is_some_and(|session| {
        session.flash_messages.push(msg.flash.clone());
        true
    })
}

/// Take and clear flash messages from a session.
fn take_flashes_from_session(
    sessions: &mut HashMap<String, SessionData>,
    session_id: &str,
) -> Vec<FlashMessage> {
    sessions
        .get_mut(session_id)
        .map(|session| std::mem::take(&mut session.flash_messages))
        .unwrap_or_default()
}

// ============================================================================
// Messages
// ============================================================================

/// Create a new session.
#[derive(Clone, Debug)]
pub struct CreateSession {
    /// User ID to associate with the session.
    pub user_id: Option<i64>,
    /// Session TTL in seconds.
    pub ttl_seconds: u64,
    /// Initial data for the session.
    pub initial_data: std::collections::HashMap<String, String>,
    /// Response channel for the created session.
    pub response_tx: Option<ResponseChannel<SessionData>>,
}

impl CreateSession {
    /// Create a new create session request with response channel.
    #[must_use]
    pub fn with_response(
        user_id: Option<i64>,
        ttl_seconds: u64,
    ) -> (Self, oneshot::Receiver<SessionData>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            user_id,
            ttl_seconds,
            initial_data: std::collections::HashMap::new(),
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Load a session by ID.
#[derive(Clone, Debug)]
pub struct LoadSession {
    /// Session ID to load.
    pub session_id: String,
    /// Response channel.
    pub response_tx: Option<ResponseChannel<Option<SessionData>>>,
}

impl LoadSession {
    /// Create a new load session request with response channel.
    #[must_use]
    pub fn with_response(session_id: String) -> (Self, oneshot::Receiver<Option<SessionData>>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Update a session.
#[derive(Clone, Debug)]
pub struct UpdateSession {
    /// Session ID to update.
    pub session_id: String,
    /// Data to merge into the session.
    pub data: std::collections::HashMap<String, String>,
    /// Optional user ID to set.
    pub user_id: Option<i64>,
    /// Response channel.
    pub response_tx: Option<ResponseChannel<Option<SessionData>>>,
}

impl UpdateSession {
    /// Create a new update session request with response channel.
    #[must_use]
    pub fn with_response(
        session_id: String,
        data: std::collections::HashMap<String, String>,
        user_id: Option<i64>,
    ) -> (Self, oneshot::Receiver<Option<SessionData>>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            data,
            user_id,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Delete a session.
#[derive(Clone, Debug)]
pub struct DeleteSession {
    /// Session ID to delete.
    pub session_id: String,
    /// Response channel.
    pub response_tx: Option<ResponseChannel<bool>>,
}

impl DeleteSession {
    /// Create a new delete session request with response channel.
    #[must_use]
    pub fn with_response(session_id: String) -> (Self, oneshot::Receiver<bool>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Add a flash message to a session.
#[derive(Clone, Debug)]
pub struct AddFlash {
    /// Session ID.
    pub session_id: String,
    /// Flash message to add.
    pub flash: FlashMessage,
    /// Response channel.
    pub response_tx: Option<ResponseChannel<bool>>,
}

impl AddFlash {
    /// Create a new add flash request with response channel.
    #[must_use]
    pub fn with_response(
        session_id: String,
        flash: FlashMessage,
    ) -> (Self, oneshot::Receiver<bool>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            flash,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Take and clear flash messages from a session.
#[derive(Clone, Debug)]
pub struct TakeFlashes {
    /// Session ID.
    pub session_id: String,
    /// Response channel.
    pub response_tx: Option<ResponseChannel<Vec<FlashMessage>>>,
}

impl TakeFlashes {
    /// Create a new take flashes request with response channel.
    #[must_use]
    pub fn with_response(session_id: String) -> (Self, oneshot::Receiver<Vec<FlashMessage>>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            session_id,
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Trigger cleanup of expired sessions.
#[derive(Clone, Debug)]
pub struct CleanupExpired;

#[cfg(test)]
mod tests {
    use super::*;
    use acton_reactive::prelude::ActorHandleInterface;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_create_and_load_session() {
        let mut runtime = ActonApp::launch_async().await;
        let agent = SessionManagerAgent::spawn(&mut runtime, 300).await.unwrap();

        // Create a session
        let (request, rx) = CreateSession::with_response(Some(123), 3600);
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let session = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert_eq!(session.user_id, Some(123));
        let session_id = session.session_id.clone();

        // Load the session
        let (request, rx) = LoadSession::with_response(session_id);
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let loaded = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().user_id, Some(123));

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_delete_session() {
        let mut runtime = ActonApp::launch_async().await;
        let agent = SessionManagerAgent::spawn(&mut runtime, 300).await.unwrap();

        // Create a session
        let (request, rx) = CreateSession::with_response(None, 3600);
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let session = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        let session_id = session.session_id.clone();

        // Delete the session
        let (request, rx) = DeleteSession::with_response(session_id.clone());
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let deleted = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert!(deleted);

        // Verify it's gone
        let (request, rx) = LoadSession::with_response(session_id);
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let loaded = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert!(loaded.is_none());

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_flash_messages() {
        let mut runtime = ActonApp::launch_async().await;
        let agent = SessionManagerAgent::spawn(&mut runtime, 300).await.unwrap();

        // Create a session
        let (request, rx) = CreateSession::with_response(None, 3600);
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let session = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        let session_id = session.session_id.clone();

        // Add flash messages
        let (request, rx) = AddFlash::with_response(
            session_id.clone(),
            FlashMessage {
                level: "success".to_string(),
                message: "Test message".to_string(),
            },
        );
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let added = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert!(added);

        // Take flashes
        let (request, rx) = TakeFlashes::with_response(session_id.clone());
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let flashes = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert_eq!(flashes.len(), 1);
        assert_eq!(flashes[0].message, "Test message");

        // Verify flashes are cleared
        let (request, rx) = TakeFlashes::with_response(session_id);
        agent.send(request).await;

        // Allow message processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        let flashes = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout")
            .expect("Channel closed");

        assert!(flashes.is_empty());

        runtime.shutdown_all().await.expect("Failed to shutdown");
    }
}

//! OAuth2 state management agent
//!
//! This module provides an acton-reactive agent for managing OAuth2 state tokens
//! and preventing CSRF attacks during the OAuth2 flow.

use acton_reactive::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{oneshot, Mutex};

use super::types::{OAuthProvider, OAuthState};

/// Type alias for response channels (web handler pattern)
pub type ResponseChannel<T> = Arc<Mutex<Option<oneshot::Sender<T>>>>;

/// OAuth2 state management agent
///
/// This agent stores and validates OAuth2 state tokens to prevent CSRF attacks.
/// State tokens are ephemeral and expire after 10 minutes.
#[derive(Debug, Default, Clone)]
pub struct OAuth2Agent {
    /// Map of state tokens to their metadata
    states: HashMap<String, OAuthState>,
}

impl OAuth2Agent {
    /// Clean up expired state tokens
    fn cleanup_expired(&mut self) {
        let now = SystemTime::now();
        self.states.retain(|_, state| state.expires_at > now);
    }
}

/// Message to generate a new OAuth2 state token (web handler)
#[derive(Debug, Clone)]
pub struct GenerateState {
    /// Provider for this state
    pub provider: OAuthProvider,
    /// Response channel
    pub response_tx: ResponseChannel<OAuthState>,
}

impl GenerateState {
    /// Create a new generate state request with response channel
    #[must_use]
    pub fn new(provider: OAuthProvider) -> (Self, oneshot::Receiver<OAuthState>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                provider,
                response_tx: Arc::new(Mutex::new(Some(tx))),
            },
            rx,
        )
    }
}

/// Message to validate an OAuth2 state token (web handler)
#[derive(Debug, Clone)]
pub struct ValidateState {
    /// State token to validate
    pub token: String,
    /// Response channel
    pub response_tx: ResponseChannel<Option<OAuthState>>,
}

impl ValidateState {
    /// Create a new validate state request with response channel
    #[must_use]
    pub fn new(token: String) -> (Self, oneshot::Receiver<Option<OAuthState>>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                token,
                response_tx: Arc::new(Mutex::new(Some(tx))),
            },
            rx,
        )
    }
}

/// Message to remove a state token (after successful use)
#[derive(Debug, Clone)]
pub struct RemoveState {
    /// State token to remove
    pub token: String,
}

/// Message to clean up expired state tokens
#[derive(Debug, Clone)]
pub struct CleanupExpired;

impl OAuth2Agent {
    /// Spawn OAuth2 manager actor
    ///
    /// # Errors
    ///
    /// Returns error if actor configuration or spawning fails
    pub async fn spawn(runtime: &mut ActorRuntime) -> anyhow::Result<ActorHandle> {
        let config = ActorConfig::new(Ern::with_root("oauth2_manager")?, None, None)?;

        let mut builder = runtime.new_actor_with_config::<Self>(config);

        // Configure handlers using mutate_on (all operations mutate state)
        builder
            .mutate_on::<GenerateState>(|actor, context| {
                let response_tx = context.message().response_tx.clone();
                let provider = context.message().provider;

                // Clean up expired tokens periodically
                actor.model.cleanup_expired();

                // Generate and store state token
                let state = OAuthState::generate(provider);
                actor.model.states.insert(state.token.clone(), state.clone());

                tracing::debug!(
                    provider = ?provider,
                    token = %state.token,
                    "Generated OAuth2 state token"
                );

                Reply::pending(async move {
                    let mut guard = response_tx.lock().await;
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(state);
                    }
                })
            })
            .mutate_on::<ValidateState>(|actor, context| {
                let token = context.message().token.clone();
                let response_tx = context.message().response_tx.clone();

                // Clean up expired tokens
                actor.model.cleanup_expired();

                // Validate state token
                let state = actor.model.states.get(&token).and_then(|state| {
                    if state.is_expired() {
                        tracing::warn!(token = %token, "OAuth2 state token expired");
                        None
                    } else {
                        tracing::debug!(
                            token = %token,
                            provider = ?state.provider,
                            "Validated OAuth2 state token"
                        );
                        Some(state.clone())
                    }
                });

                Reply::pending(async move {
                    let mut guard = response_tx.lock().await;
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(state);
                    }
                })
            })
            .mutate_on::<RemoveState>(|actor, context| {
                let token = context.message().token.clone();

                if actor.model.states.remove(&token).is_some() {
                    tracing::debug!(token = %token, "Removed OAuth2 state token");
                }

                Reply::ready()
            })
            .mutate_on::<CleanupExpired>(|actor, _context| {
                let before = actor.model.states.len();
                actor.model.cleanup_expired();
                let removed = before - actor.model.states.len();

                if removed > 0 {
                    tracing::debug!(
                        removed = removed,
                        remaining = actor.model.states.len(),
                        "Cleaned up expired OAuth2 state tokens"
                    );
                }

                Reply::ready()
            })
            .after_start(|_actor| async {
                tracing::info!("OAuth2 manager actor started");
            })
            .after_stop(|actor| {
                let token_count = actor.model.states.len();
                async move {
                    tracing::info!(
                        tokens = token_count,
                        "OAuth2 manager actor stopped"
                    );
                }
            });

        Ok(builder.start().await)
    }
}

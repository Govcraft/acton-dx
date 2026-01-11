//! Hot Reload Coordinator Agent
//!
//! Actor-based hot reload management using acton-reactive.
//! Coordinates file watching, debouncing, and reload broadcasting.
//!
//! This agent supervises file watchers and coordinates reloads for:
//! - Templates (Askama templates)
//! - Configuration files
//! - Cedar policies
//!
//! Features:
//! - Debouncing to prevent excessive reloads
//! - Graceful shutdown of watchers
//! - Broadcast notifications for reloads

use crate::htmx::agents::default_actor_config;
use crate::htmx::agents::request_reply::{create_request_reply, send_response, ResponseChannel};
use acton_reactive::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, oneshot};

/// Default debounce duration for file changes
const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_millis(100);

/// Type of resource that can be hot-reloaded
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReloadType {
    /// Template files (HTML, Askama)
    Templates,
    /// Configuration files (TOML, YAML, JSON)
    Config,
    /// Cedar policy files
    Policies,
    /// Static assets
    Assets,
}

impl ReloadType {
    /// Get all reload types
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Templates, Self::Config, Self::Policies, Self::Assets]
    }

    /// Get the display name for this reload type
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Templates => "templates",
            Self::Config => "config",
            Self::Policies => "policies",
            Self::Assets => "assets",
        }
    }
}

impl std::fmt::Display for ReloadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A reload event that can be broadcast to subscribers
#[derive(Debug, Clone)]
pub struct ReloadEvent {
    /// Type of reload that occurred
    pub reload_type: ReloadType,
    /// Paths that triggered the reload
    pub paths: Vec<PathBuf>,
    /// Timestamp of the reload
    pub timestamp: Instant,
}

impl ReloadEvent {
    /// Create a new reload event
    #[must_use]
    pub fn new(reload_type: ReloadType, paths: Vec<PathBuf>) -> Self {
        Self {
            reload_type,
            paths,
            timestamp: Instant::now(),
        }
    }
}

/// Configuration for hot reload behavior
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// Debounce duration for each reload type
    pub debounce: HashMap<ReloadType, Duration>,
    /// Watch paths for each reload type
    pub watch_paths: HashMap<ReloadType, Vec<PathBuf>>,
    /// Whether hot reload is enabled
    pub enabled: bool,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        let mut debounce = HashMap::new();
        for rt in ReloadType::all() {
            debounce.insert(*rt, DEFAULT_DEBOUNCE_DURATION);
        }

        Self {
            debounce,
            watch_paths: HashMap::new(),
            enabled: true,
        }
    }
}

impl HotReloadConfig {
    /// Create a new hot reload configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set debounce duration for a reload type
    #[must_use]
    pub fn with_debounce(mut self, reload_type: ReloadType, duration: Duration) -> Self {
        self.debounce.insert(reload_type, duration);
        self
    }

    /// Add watch paths for a reload type
    #[must_use]
    pub fn with_watch_paths(
        mut self,
        reload_type: ReloadType,
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        self.watch_paths
            .entry(reload_type)
            .or_default()
            .extend(paths);
        self
    }

    /// Enable or disable hot reload
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Get the debounce duration for a reload type
    #[must_use]
    pub fn debounce_for(&self, reload_type: ReloadType) -> Duration {
        self.debounce
            .get(&reload_type)
            .copied()
            .unwrap_or(DEFAULT_DEBOUNCE_DURATION)
    }
}

/// Pending file change information for debouncing
#[derive(Debug, Clone)]
struct PendingChange {
    /// Paths that have changed
    paths: HashSet<PathBuf>,
    /// When the last change was detected
    last_change: Instant,
}

impl PendingChange {
    fn new(path: PathBuf) -> Self {
        let mut paths = HashSet::new();
        paths.insert(path);
        Self {
            paths,
            last_change: Instant::now(),
        }
    }

    fn add_path(&mut self, path: PathBuf) {
        self.paths.insert(path);
        self.last_change = Instant::now();
    }

    fn should_trigger(&self, debounce: Duration) -> bool {
        self.last_change.elapsed() >= debounce
    }

    fn into_paths(self) -> Vec<PathBuf> {
        self.paths.into_iter().collect()
    }
}

// Type alias for the actor builder
type HotReloadActorBuilder = ManagedActor<Idle, HotReloadCoordinatorAgent>;

/// Hot reload coordinator agent model
#[derive(Debug)]
pub struct HotReloadCoordinatorAgent {
    /// Configuration for hot reload behavior
    config: HotReloadConfig,
    /// Pending changes per reload type (for debouncing)
    pending_changes: HashMap<ReloadType, PendingChange>,
    /// Broadcast sender for reload events
    reload_tx: broadcast::Sender<ReloadEvent>,
    /// Number of reload events emitted
    reload_count: u64,
}

impl Default for HotReloadCoordinatorAgent {
    fn default() -> Self {
        Self::new(HotReloadConfig::default())
    }
}

impl Clone for HotReloadCoordinatorAgent {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pending_changes: HashMap::new(),
            reload_tx: self.reload_tx.clone(),
            reload_count: self.reload_count,
        }
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// Report a file change (from external file watcher)
#[derive(Clone, Debug)]
pub struct FileChanged {
    /// Type of file that changed
    pub reload_type: ReloadType,
    /// Path to the changed file
    pub path: PathBuf,
}

impl FileChanged {
    /// Create a new file changed message
    #[must_use]
    pub const fn new(reload_type: ReloadType, path: PathBuf) -> Self {
        Self { reload_type, path }
    }
}

/// Force an immediate reload (bypass debouncing)
#[derive(Clone, Debug)]
pub struct ForceReload {
    /// Type to reload
    pub reload_type: ReloadType,
}

impl ForceReload {
    /// Create a new force reload message
    #[must_use]
    pub const fn new(reload_type: ReloadType) -> Self {
        Self { reload_type }
    }
}

/// Trigger debounced reloads (sent periodically by timer)
#[derive(Clone, Debug)]
pub struct TriggerPendingReloads;

/// Request to subscribe to reload events
#[derive(Clone, Debug, Default)]
pub struct Subscribe {
    /// Optional response channel for web handlers
    pub response_tx: Option<ResponseChannel<broadcast::Receiver<ReloadEvent>>>,
}

impl Subscribe {
    /// Create a new subscribe request with response channel
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<broadcast::Receiver<ReloadEvent>>) {
        let (response_tx, rx) = create_request_reply();
        let request = Self {
            response_tx: Some(response_tx),
        };
        (request, rx)
    }
}

/// Get current reload statistics
#[derive(Clone, Debug, Default)]
pub struct GetStats {
    /// Optional response channel
    pub response_tx: Option<ResponseChannel<HotReloadStats>>,
}

impl GetStats {
    /// Create a new get stats request
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<HotReloadStats>) {
        let (response_tx, rx) = create_request_reply();
        (Self { response_tx: Some(response_tx) }, rx)
    }
}

/// Hot reload statistics
#[derive(Clone, Debug, Default)]
pub struct HotReloadStats {
    /// Total reload events emitted
    pub reload_count: u64,
    /// Number of pending changes per type
    pub pending_counts: HashMap<ReloadType, usize>,
    /// Whether hot reload is enabled
    pub enabled: bool,
}

/// Update hot reload configuration
#[derive(Clone, Debug)]
pub struct UpdateConfig {
    /// New configuration
    pub config: HotReloadConfig,
}

impl UpdateConfig {
    /// Create a new update config message
    #[must_use]
    pub const fn new(config: HotReloadConfig) -> Self {
        Self { config }
    }
}

impl HotReloadCoordinatorAgent {
    /// Create a new hot reload coordinator with the given configuration
    #[must_use]
    pub fn new(config: HotReloadConfig) -> Self {
        let (reload_tx, _) = broadcast::channel(64);
        Self {
            config,
            pending_changes: HashMap::new(),
            reload_tx,
            reload_count: 0,
        }
    }

    /// Spawn hot reload coordinator actor
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails
    pub async fn spawn(runtime: &mut ActorRuntime) -> anyhow::Result<ActorHandle> {
        Self::spawn_with_config(runtime, HotReloadConfig::default()).await
    }

    /// Spawn hot reload coordinator actor with custom configuration
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails
    pub async fn spawn_with_config(
        runtime: &mut ActorRuntime,
        config: HotReloadConfig,
    ) -> anyhow::Result<ActorHandle> {
        let actor_config = default_actor_config("hot_reload_coordinator")?;
        let mut builder = runtime.new_actor_with_config::<Self>(actor_config);

        // Update the model with the custom configuration
        builder.model.config = config;

        Self::configure_handlers(builder).await
    }

    /// Configure all message handlers
    async fn configure_handlers(mut builder: HotReloadActorBuilder) -> anyhow::Result<ActorHandle> {
        builder
            // Handle file change events
            .mutate_on::<FileChanged>(|actor, context| {
                let reload_type = context.message().reload_type;
                let path = context.message().path.clone();

                if actor.model.config.enabled {
                    Self::record_file_change(&mut actor.model, reload_type, path);
                }

                Reply::ready()
            })
            // Handle force reload
            .mutate_on::<ForceReload>(|actor, context| {
                let reload_type = context.message().reload_type;
                let reload_tx = actor.model.reload_tx.clone();

                if actor.model.config.enabled {
                    // Take any pending paths for this type
                    let paths = actor
                        .model
                        .pending_changes
                        .remove(&reload_type)
                        .map(PendingChange::into_paths)
                        .unwrap_or_default();

                    actor.model.reload_count += 1;
                    let event = ReloadEvent::new(reload_type, paths);

                    Reply::pending(async move {
                        let _ = reload_tx.send(event);
                    })
                } else {
                    Reply::ready()
                }
            })
            // Handle periodic trigger for pending reloads
            .mutate_on::<TriggerPendingReloads>(|actor, _context| {
                if !actor.model.config.enabled {
                    return Reply::ready();
                }

                let mut events_to_send = Vec::new();

                // Check each pending change type
                let reload_types: Vec<_> = actor.model.pending_changes.keys().copied().collect();
                for reload_type in reload_types {
                    let debounce = actor.model.config.debounce_for(reload_type);

                    if let Some(pending) = actor.model.pending_changes.get(&reload_type) {
                        if pending.should_trigger(debounce) {
                            // Time to trigger this reload
                            if let Some(pending) = actor.model.pending_changes.remove(&reload_type)
                            {
                                actor.model.reload_count += 1;
                                events_to_send.push(ReloadEvent::new(
                                    reload_type,
                                    pending.into_paths(),
                                ));
                            }
                        }
                    }
                }

                if events_to_send.is_empty() {
                    Reply::ready()
                } else {
                    let reload_tx = actor.model.reload_tx.clone();
                    Reply::pending(async move {
                        for event in events_to_send {
                            let _ = reload_tx.send(event);
                        }
                    })
                }
            })
            // Handle subscribe requests
            .mutate_on::<Subscribe>(|actor, context| {
                let response_tx = context.message().response_tx.clone();
                let rx = actor.model.reload_tx.subscribe();

                if let Some(tx) = response_tx {
                    Reply::pending(async move {
                        let _ = send_response(tx, rx).await;
                    })
                } else {
                    Reply::ready()
                }
            })
            // Handle get stats requests
            .mutate_on::<GetStats>(|actor, context| {
                let response_tx = context.message().response_tx.clone();

                let stats = HotReloadStats {
                    reload_count: actor.model.reload_count,
                    pending_counts: actor
                        .model
                        .pending_changes
                        .iter()
                        .map(|(k, v)| (*k, v.paths.len()))
                        .collect(),
                    enabled: actor.model.config.enabled,
                };

                if let Some(tx) = response_tx {
                    Reply::pending(async move {
                        let _ = send_response(tx, stats).await;
                    })
                } else {
                    Reply::ready()
                }
            })
            // Handle config updates
            .mutate_on::<UpdateConfig>(|actor, context| {
                actor.model.config = context.message().config.clone();
                tracing::info!("Hot reload configuration updated");
                Reply::ready()
            });

        Ok(builder.start().await)
    }

    /// Record a file change (internal helper)
    fn record_file_change(model: &mut Self, reload_type: ReloadType, path: PathBuf) {
        model
            .pending_changes
            .entry(reload_type)
            .and_modify(|pending| pending.add_path(path.clone()))
            .or_insert_with(|| PendingChange::new(path));

        tracing::debug!(
            reload_type = %reload_type,
            "File change recorded, pending debounce"
        );
    }

    /// Get a receiver for reload events
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ReloadEvent> {
        self.reload_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reload_type_all() {
        let all = ReloadType::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_reload_type_display() {
        assert_eq!(format!("{}", ReloadType::Templates), "templates");
        assert_eq!(format!("{}", ReloadType::Config), "config");
        assert_eq!(format!("{}", ReloadType::Policies), "policies");
        assert_eq!(format!("{}", ReloadType::Assets), "assets");
    }

    #[test]
    fn test_reload_event_creation() {
        let event = ReloadEvent::new(ReloadType::Templates, vec![PathBuf::from("test.html")]);
        assert_eq!(event.reload_type, ReloadType::Templates);
        assert_eq!(event.paths.len(), 1);
    }

    #[test]
    fn test_hot_reload_config_default() {
        let config = HotReloadConfig::default();
        assert!(config.enabled);
        assert_eq!(
            config.debounce_for(ReloadType::Templates),
            DEFAULT_DEBOUNCE_DURATION
        );
    }

    #[test]
    fn test_hot_reload_config_builder() {
        let config = HotReloadConfig::new()
            .with_debounce(ReloadType::Templates, Duration::from_millis(200))
            .with_watch_paths(
                ReloadType::Templates,
                vec![PathBuf::from("templates/")],
            )
            .with_enabled(false);

        assert!(!config.enabled);
        assert_eq!(
            config.debounce_for(ReloadType::Templates),
            Duration::from_millis(200)
        );
        assert!(config.watch_paths.contains_key(&ReloadType::Templates));
    }

    #[test]
    fn test_pending_change_debounce() {
        let change = PendingChange::new(PathBuf::from("test.html"));
        // Immediately after creation, should not trigger
        assert!(!change.should_trigger(Duration::from_millis(100)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_hot_reload_agent_spawn() {
        let mut runtime = ActonApp::launch_async().await;
        let result = HotReloadCoordinatorAgent::spawn(&mut runtime).await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_hot_reload_subscribe() {
        let mut runtime = ActonApp::launch_async().await;
        let handle = HotReloadCoordinatorAgent::spawn(&mut runtime).await.unwrap();

        let (request, rx) = Subscribe::new();
        handle.send(request).await;

        let subscriber = rx.await.expect("Failed to get subscriber");
        // Subscriber should be valid
        assert!(subscriber.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_hot_reload_stats() {
        let mut runtime = ActonApp::launch_async().await;
        let handle = HotReloadCoordinatorAgent::spawn(&mut runtime).await.unwrap();

        let (request, rx) = GetStats::new();
        handle.send(request).await;

        let stats = rx.await.expect("Failed to get stats");
        assert_eq!(stats.reload_count, 0);
        assert!(stats.enabled);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_hot_reload_file_change_and_force_reload() {
        let mut runtime = ActonApp::launch_async().await;
        let handle = HotReloadCoordinatorAgent::spawn(&mut runtime).await.unwrap();

        // Subscribe to events
        let (subscribe_req, subscribe_rx) = Subscribe::new();
        handle.send(subscribe_req).await;
        let mut subscriber = subscribe_rx.await.expect("Failed to subscribe");

        // Report a file change
        handle
            .send(FileChanged::new(
                ReloadType::Templates,
                PathBuf::from("test.html"),
            ))
            .await;

        // Force reload
        handle.send(ForceReload::new(ReloadType::Templates)).await;

        // Wait a bit for the event to be processed
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Should receive the reload event
        let event = subscriber.try_recv();
        assert!(event.is_ok());
        let event = event.unwrap();
        assert_eq!(event.reload_type, ReloadType::Templates);
    }
}

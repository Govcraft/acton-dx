//! Service Coordinator Agent
//!
//! Actor-based service client management using acton-reactive.
//! Coordinates service health monitoring, circuit breakers, and reconnection.
//!
//! This agent manages connections to external microservices:
//! - Auth service
//! - Data service
//! - Cedar service
//! - Cache service
//! - Email service
//! - File service
//!
//! Features:
//! - Health monitoring with circuit breakers
//! - Automatic reconnection on service restart
//! - Status event broadcasting
//! - Graceful degradation

use crate::htmx::agents::default_actor_config;
use crate::htmx::agents::request_reply::{create_request_reply, send_response, ResponseChannel};
use acton_reactive::prelude::*;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, oneshot};

/// Type alias for boxed futures used in message handler returns
type FutureBox = Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>>;

/// Default health check interval
const DEFAULT_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Default circuit breaker failure threshold
const DEFAULT_FAILURE_THRESHOLD: u32 = 5;

/// Default circuit breaker recovery timeout
const DEFAULT_RECOVERY_TIMEOUT: Duration = Duration::from_secs(60);

/// Service identifier for microservices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceId {
    /// Authentication service
    Auth,
    /// Data/database service
    Data,
    /// Cedar policy service
    Cedar,
    /// Cache service
    Cache,
    /// Email service
    Email,
    /// File storage service
    File,
}

impl ServiceId {
    /// Get all service IDs
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Auth,
            Self::Data,
            Self::Cedar,
            Self::Cache,
            Self::Email,
            Self::File,
        ]
    }

    /// Get the display name for this service
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Data => "data",
            Self::Cedar => "cedar",
            Self::Cache => "cache",
            Self::Email => "email",
            Self::File => "file",
        }
    }

    /// Get the default port for this service
    #[must_use]
    pub const fn default_port(&self) -> u16 {
        match self {
            Self::Auth => 50051,
            Self::Data => 50052,
            Self::Cedar => 50053,
            Self::Cache => 50054,
            Self::Email => 50055,
            Self::File => 50056,
        }
    }
}

impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Current state of a service
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Service is healthy and accepting requests
    Healthy,
    /// Service is experiencing issues but still responding
    Degraded,
    /// Service is not responding (circuit breaker open)
    Unhealthy,
    /// Service state is unknown (not yet checked)
    Unknown,
}

impl std::fmt::Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed - requests flow normally
    Closed,
    /// Circuit is open - requests are blocked
    Open,
    /// Circuit is half-open - allowing test requests
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "closed"),
            Self::Open => write!(f, "open"),
            Self::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Circuit breaker for a single service
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Current state of the circuit
    pub state: CircuitState,
    /// Number of consecutive failures
    pub failure_count: u32,
    /// Failure threshold before opening
    pub failure_threshold: u32,
    /// Time when circuit was opened
    pub opened_at: Option<Instant>,
    /// Time to wait before attempting recovery
    pub recovery_timeout: Duration,
    /// Last successful request time
    pub last_success: Option<Instant>,
    /// Last failure time
    pub last_failure: Option<Instant>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(DEFAULT_FAILURE_THRESHOLD, DEFAULT_RECOVERY_TIMEOUT)
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given threshold and timeout
    #[must_use]
    pub const fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            opened_at: None,
            recovery_timeout,
            last_success: None,
            last_failure: None,
        }
    }

    /// Record a successful request
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.last_success = Some(Instant::now());
        self.state = CircuitState::Closed;
        self.opened_at = None;
    }

    /// Record a failed request
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());

        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
            self.opened_at = Some(Instant::now());
        }
    }

    /// Check if the circuit should allow a request
    #[must_use]
    pub fn should_allow(&mut self) -> bool {
        match self.state {
            CircuitState::Open => {
                if let Some(opened_at) = self.opened_at {
                    if opened_at.elapsed() >= self.recovery_timeout {
                        self.state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    // No opened_at timestamp, transition to half-open
                    self.state = CircuitState::HalfOpen;
                    true
                }
            }
            // Both Closed and HalfOpen allow requests through
            CircuitState::Closed | CircuitState::HalfOpen => true,
        }
    }

    /// Get the current state of the circuit breaker
    #[must_use]
    pub const fn state(&self) -> CircuitState {
        self.state
    }
}

/// Service health status
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    /// Service identifier
    pub service_id: ServiceId,
    /// Current service state
    pub state: ServiceState,
    /// Circuit breaker state
    pub circuit: CircuitBreaker,
    /// Endpoint URL
    pub endpoint: String,
    /// Last health check time
    pub last_check: Option<Instant>,
    /// Response time of last successful check (milliseconds)
    pub response_time_ms: Option<u64>,
}

impl ServiceHealth {
    /// Create new service health entry
    #[must_use]
    pub fn new(service_id: ServiceId, endpoint: String) -> Self {
        Self {
            service_id,
            state: ServiceState::Unknown,
            circuit: CircuitBreaker::default(),
            endpoint,
            last_check: None,
            response_time_ms: None,
        }
    }
}

/// A service status change event
#[derive(Debug, Clone)]
pub struct ServiceStatusEvent {
    /// Service that changed
    pub service_id: ServiceId,
    /// Previous state
    pub previous_state: ServiceState,
    /// New state
    pub new_state: ServiceState,
    /// Timestamp of change
    pub timestamp: Instant,
}

/// Configuration for service coordinator
#[derive(Debug, Clone)]
pub struct ServiceCoordinatorConfig {
    /// Health check interval per service
    pub health_check_interval: Duration,
    /// Circuit breaker failure threshold
    pub failure_threshold: u32,
    /// Circuit breaker recovery timeout
    pub recovery_timeout: Duration,
    /// Service endpoints (service_id -> endpoint URL)
    pub endpoints: HashMap<ServiceId, String>,
    /// Whether health checking is enabled
    pub enabled: bool,
}

impl Default for ServiceCoordinatorConfig {
    fn default() -> Self {
        let mut endpoints = HashMap::new();
        for service in ServiceId::all() {
            endpoints.insert(
                *service,
                format!("http://127.0.0.1:{}", service.default_port()),
            );
        }

        Self {
            health_check_interval: DEFAULT_HEALTH_CHECK_INTERVAL,
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            recovery_timeout: DEFAULT_RECOVERY_TIMEOUT,
            endpoints,
            enabled: true,
        }
    }
}

impl ServiceCoordinatorConfig {
    /// Create a new configuration with defaults
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set health check interval
    #[must_use]
    pub const fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// Set circuit breaker failure threshold
    #[must_use]
    pub const fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set circuit breaker recovery timeout
    #[must_use]
    pub const fn with_recovery_timeout(mut self, timeout: Duration) -> Self {
        self.recovery_timeout = timeout;
        self
    }

    /// Set endpoint for a specific service
    #[must_use]
    pub fn with_endpoint(mut self, service_id: ServiceId, endpoint: String) -> Self {
        self.endpoints.insert(service_id, endpoint);
        self
    }

    /// Enable or disable health checking
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

// Type alias for the actor builder
type ServiceCoordinatorActorBuilder = ManagedActor<Idle, ServiceCoordinatorAgent>;

/// Service coordinator agent model
#[derive(Debug)]
pub struct ServiceCoordinatorAgent {
    /// Configuration
    config: ServiceCoordinatorConfig,
    /// Health status per service
    services: HashMap<ServiceId, ServiceHealth>,
    /// Broadcast sender for status events
    status_tx: broadcast::Sender<ServiceStatusEvent>,
    /// Total health checks performed
    health_check_count: u64,
}

impl Default for ServiceCoordinatorAgent {
    fn default() -> Self {
        Self::new(ServiceCoordinatorConfig::default())
    }
}

impl Clone for ServiceCoordinatorAgent {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            services: self.services.clone(),
            status_tx: self.status_tx.clone(),
            health_check_count: self.health_check_count,
        }
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// Report health check result for a service
#[derive(Clone, Debug)]
pub struct HealthCheckResult {
    /// Service that was checked
    pub service_id: ServiceId,
    /// Whether the check succeeded
    pub healthy: bool,
    /// Response time in milliseconds (if successful)
    pub response_time_ms: Option<u64>,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl HealthCheckResult {
    /// Create a successful health check result
    #[must_use]
    pub const fn success(service_id: ServiceId, response_time_ms: u64) -> Self {
        Self {
            service_id,
            healthy: true,
            response_time_ms: Some(response_time_ms),
            error: None,
        }
    }

    /// Create a failed health check result
    #[must_use]
    pub const fn failure(service_id: ServiceId, error: String) -> Self {
        Self {
            service_id,
            healthy: false,
            response_time_ms: None,
            error: Some(error),
        }
    }
}

/// Mark a service as available (e.g., after successful request)
#[derive(Clone, Debug)]
pub struct ServiceAvailable {
    /// Service that is available
    pub service_id: ServiceId,
}

impl ServiceAvailable {
    /// Create a new message
    #[must_use]
    pub const fn new(service_id: ServiceId) -> Self {
        Self { service_id }
    }
}

/// Mark a service as unavailable (e.g., after failed request)
#[derive(Clone, Debug)]
pub struct ServiceUnavailable {
    /// Service that is unavailable
    pub service_id: ServiceId,
    /// Error message
    pub error: Option<String>,
}

impl ServiceUnavailable {
    /// Create a new message
    #[must_use]
    pub const fn new(service_id: ServiceId, error: Option<String>) -> Self {
        Self { service_id, error }
    }
}

/// Subscribe to service status events
#[derive(Clone, Debug, Default)]
pub struct Subscribe {
    /// Optional response channel
    pub response_tx: Option<ResponseChannel<broadcast::Receiver<ServiceStatusEvent>>>,
}

impl Subscribe {
    /// Create a new subscribe request
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<broadcast::Receiver<ServiceStatusEvent>>) {
        let (response_tx, rx) = create_request_reply();
        (
            Self {
                response_tx: Some(response_tx),
            },
            rx,
        )
    }
}

/// Get current service status
#[derive(Clone, Debug, Default)]
pub struct GetServiceStatus {
    /// Optional response channel
    pub response_tx: Option<ResponseChannel<ServiceStatusResponse>>,
}

impl GetServiceStatus {
    /// Create a new get status request
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<ServiceStatusResponse>) {
        let (response_tx, rx) = create_request_reply();
        (
            Self {
                response_tx: Some(response_tx),
            },
            rx,
        )
    }
}

/// Response with all service statuses
#[derive(Clone, Debug, Default)]
pub struct ServiceStatusResponse {
    /// Status per service
    pub services: HashMap<ServiceId, (ServiceState, CircuitState)>,
    /// Total health checks performed
    pub health_check_count: u64,
    /// Whether health checking is enabled
    pub enabled: bool,
}

/// Update service coordinator configuration
#[derive(Clone, Debug)]
pub struct UpdateConfig {
    /// New configuration
    pub config: ServiceCoordinatorConfig,
}

impl UpdateConfig {
    /// Create a new update config message
    #[must_use]
    pub const fn new(config: ServiceCoordinatorConfig) -> Self {
        Self { config }
    }
}

impl ServiceCoordinatorAgent {
    /// Create a new service coordinator with the given configuration
    #[must_use]
    pub fn new(config: ServiceCoordinatorConfig) -> Self {
        let (status_tx, _) = broadcast::channel(64);

        let mut services = HashMap::new();
        for (service_id, endpoint) in &config.endpoints {
            services.insert(*service_id, ServiceHealth::new(*service_id, endpoint.clone()));
        }

        Self {
            config,
            services,
            status_tx,
            health_check_count: 0,
        }
    }

    /// Spawn service coordinator actor
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails
    pub async fn spawn(runtime: &mut ActorRuntime) -> anyhow::Result<ActorHandle> {
        Self::spawn_with_config(runtime, ServiceCoordinatorConfig::default()).await
    }

    /// Spawn service coordinator actor with custom configuration
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails
    pub async fn spawn_with_config(
        runtime: &mut ActorRuntime,
        config: ServiceCoordinatorConfig,
    ) -> anyhow::Result<ActorHandle> {
        let actor_config = default_actor_config("service_coordinator")?;
        let mut builder = runtime.new_actor_with_config::<Self>(actor_config);

        // Update model with custom configuration
        let mut services = HashMap::new();
        for (service_id, endpoint) in &config.endpoints {
            let mut health = ServiceHealth::new(*service_id, endpoint.clone());
            // Apply config values to circuit breaker
            health.circuit.failure_threshold = config.failure_threshold;
            health.circuit.recovery_timeout = config.recovery_timeout;
            services.insert(*service_id, health);
        }
        builder.model.config = config;
        builder.model.services = services;

        Self::configure_handlers(builder).await
    }

    /// Configure all message handlers
    async fn configure_handlers(
        mut builder: ServiceCoordinatorActorBuilder,
    ) -> anyhow::Result<ActorHandle> {
        Self::configure_health_handlers(&mut builder);
        Self::configure_request_handlers(&mut builder);
        Self::configure_config_handlers(&mut builder);
        Ok(builder.start().await)
    }

    /// Configure health-related message handlers
    fn configure_health_handlers(builder: &mut ServiceCoordinatorActorBuilder) {
        builder
            .mutate_on::<HealthCheckResult>(|actor, context| {
                let result = context.message();
                actor.model.health_check_count += 1;
                let Some(health) = actor.model.services.get_mut(&result.service_id) else {
                    return Box::pin(async {}) as FutureBox;
                };
                let previous = health.state;
                health.last_check = Some(Instant::now());
                if result.healthy {
                    health.circuit.record_success();
                    health.response_time_ms = result.response_time_ms;
                    health.state = ServiceState::Healthy;
                } else {
                    health.circuit.record_failure();
                    health.state = Self::state_from_circuit(&health.circuit);
                }
                Self::maybe_broadcast(&actor.model.status_tx, result.service_id, previous, health.state)
            })
            .mutate_on::<ServiceAvailable>(|actor, context| {
                let id = context.message().service_id;
                let Some(health) = actor.model.services.get_mut(&id) else {
                    return Box::pin(async {}) as FutureBox;
                };
                let prev = health.state;
                health.circuit.record_success();
                health.state = ServiceState::Healthy;
                Self::maybe_broadcast(&actor.model.status_tx, id, prev, ServiceState::Healthy)
            })
            .mutate_on::<ServiceUnavailable>(|actor, context| {
                let id = context.message().service_id;
                let Some(health) = actor.model.services.get_mut(&id) else {
                    return Box::pin(async {}) as FutureBox;
                };
                let prev = health.state;
                health.circuit.record_failure();
                let new_state = Self::state_from_circuit(&health.circuit);
                health.state = new_state;
                Self::maybe_broadcast(&actor.model.status_tx, id, prev, new_state)
            });
    }

    /// Configure request-reply message handlers
    fn configure_request_handlers(builder: &mut ServiceCoordinatorActorBuilder) {
        builder
            .mutate_on::<Subscribe>(|actor, context| {
                let Some(tx) = context.message().response_tx.clone() else {
                    return Reply::ready();
                };
                let rx = actor.model.status_tx.subscribe();
                Reply::pending(async move { let _ = send_response(tx, rx).await; })
            })
            .mutate_on::<GetServiceStatus>(|actor, context| {
                let Some(tx) = context.message().response_tx.clone() else {
                    return Reply::ready();
                };
                let response = ServiceStatusResponse {
                    services: actor.model.services.iter()
                        .map(|(id, h)| (*id, (h.state, h.circuit.state)))
                        .collect(),
                    health_check_count: actor.model.health_check_count,
                    enabled: actor.model.config.enabled,
                };
                Reply::pending(async move { let _ = send_response(tx, response).await; })
            });
    }

    /// Configure config update handlers
    fn configure_config_handlers(builder: &mut ServiceCoordinatorActorBuilder) {
        builder.mutate_on::<UpdateConfig>(|actor, context| {
            let config = &context.message().config;
            for health in actor.model.services.values_mut() {
                health.circuit.failure_threshold = config.failure_threshold;
                health.circuit.recovery_timeout = config.recovery_timeout;
            }
            for (service_id, endpoint) in &config.endpoints {
                actor.model.services.entry(*service_id)
                    .or_insert_with(|| ServiceHealth::new(*service_id, endpoint.clone()));
            }
            actor.model.config = config.clone();
            tracing::info!("Service coordinator configuration updated");
            Reply::ready()
        });
    }

    /// Determine service state from circuit breaker state
    const fn state_from_circuit(circuit: &CircuitBreaker) -> ServiceState {
        match circuit.state {
            CircuitState::Open => ServiceState::Unhealthy,
            CircuitState::Closed | CircuitState::HalfOpen => ServiceState::Degraded,
        }
    }

    /// Broadcast status event if state changed, otherwise return ready
    fn maybe_broadcast(
        tx: &broadcast::Sender<ServiceStatusEvent>,
        id: ServiceId,
        prev: ServiceState,
        new: ServiceState,
    ) -> FutureBox {
        if prev == new {
            return Box::pin(async {});
        }
        let event = ServiceStatusEvent {
            service_id: id,
            previous_state: prev,
            new_state: new,
            timestamp: Instant::now(),
        };
        let tx = tx.clone();
        Box::pin(async move { let _ = tx.send(event); })
    }

    /// Get a receiver for status events
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ServiceStatusEvent> {
        self.status_tx.subscribe()
    }

    /// Check if a service is healthy
    #[must_use]
    pub fn is_healthy(&self, service_id: ServiceId) -> bool {
        self.services
            .get(&service_id)
            .is_some_and(|h| h.state == ServiceState::Healthy)
    }

    /// Check if a service's circuit breaker allows requests
    #[must_use]
    pub fn should_allow_request(&mut self, service_id: ServiceId) -> bool {
        self.services
            .get_mut(&service_id)
            .is_some_and(|h| h.circuit.should_allow())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_id_all() {
        let all = ServiceId::all();
        assert_eq!(all.len(), 6);
    }

    #[test]
    fn test_service_id_display() {
        assert_eq!(format!("{}", ServiceId::Auth), "auth");
        assert_eq!(format!("{}", ServiceId::Data), "data");
        assert_eq!(format!("{}", ServiceId::Cedar), "cedar");
        assert_eq!(format!("{}", ServiceId::Cache), "cache");
        assert_eq!(format!("{}", ServiceId::Email), "email");
        assert_eq!(format!("{}", ServiceId::File), "file");
    }

    #[test]
    fn test_service_id_default_port() {
        assert_eq!(ServiceId::Auth.default_port(), 50051);
        assert_eq!(ServiceId::Data.default_port(), 50052);
        assert_eq!(ServiceId::Cedar.default_port(), 50053);
        assert_eq!(ServiceId::Cache.default_port(), 50054);
        assert_eq!(ServiceId::Email.default_port(), 50055);
        assert_eq!(ServiceId::File.default_port(), 50056);
    }

    #[test]
    fn test_circuit_breaker_default() {
        let cb = CircuitBreaker::default();
        assert_eq!(cb.state, CircuitState::Closed);
        assert_eq!(cb.failure_count, 0);
        assert_eq!(cb.failure_threshold, DEFAULT_FAILURE_THRESHOLD);
    }

    #[test]
    fn test_circuit_breaker_success() {
        let mut cb = CircuitBreaker::default();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count, 2);

        cb.record_success();
        assert_eq!(cb.failure_count, 0);
        assert_eq!(cb.state, CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_opens_on_threshold() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(60));
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_should_allow() {
        let mut cb = CircuitBreaker::new(2, Duration::from_millis(10));
        assert!(cb.should_allow());

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state, CircuitState::Open);
        assert!(!cb.should_allow());

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(15));
        assert!(cb.should_allow());
        assert_eq!(cb.state, CircuitState::HalfOpen);
    }

    #[test]
    fn test_service_state_display() {
        assert_eq!(format!("{}", ServiceState::Healthy), "healthy");
        assert_eq!(format!("{}", ServiceState::Degraded), "degraded");
        assert_eq!(format!("{}", ServiceState::Unhealthy), "unhealthy");
        assert_eq!(format!("{}", ServiceState::Unknown), "unknown");
    }

    #[test]
    fn test_service_coordinator_config_default() {
        let config = ServiceCoordinatorConfig::default();
        assert!(config.enabled);
        assert_eq!(config.health_check_interval, DEFAULT_HEALTH_CHECK_INTERVAL);
        assert_eq!(config.failure_threshold, DEFAULT_FAILURE_THRESHOLD);
        assert_eq!(config.endpoints.len(), 6);
    }

    #[test]
    fn test_service_coordinator_config_builder() {
        let config = ServiceCoordinatorConfig::new()
            .with_health_check_interval(Duration::from_secs(60))
            .with_failure_threshold(10)
            .with_recovery_timeout(Duration::from_secs(120))
            .with_enabled(false);

        assert!(!config.enabled);
        assert_eq!(config.health_check_interval, Duration::from_secs(60));
        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.recovery_timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_health_check_result_success() {
        let result = HealthCheckResult::success(ServiceId::Auth, 42);
        assert!(result.healthy);
        assert_eq!(result.response_time_ms, Some(42));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_health_check_result_failure() {
        let result = HealthCheckResult::failure(ServiceId::Data, "Connection refused".to_string());
        assert!(!result.healthy);
        assert!(result.response_time_ms.is_none());
        assert_eq!(result.error, Some("Connection refused".to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_service_coordinator_spawn() {
        let mut runtime = ActonApp::launch_async().await;
        let result = ServiceCoordinatorAgent::spawn(&mut runtime).await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_service_coordinator_get_status() {
        let mut runtime = ActonApp::launch_async().await;
        let handle = ServiceCoordinatorAgent::spawn(&mut runtime).await.unwrap();

        let (request, rx) = GetServiceStatus::new();
        handle.send(request).await;

        let status = rx.await.expect("Failed to get status");
        assert!(status.enabled);
        assert_eq!(status.health_check_count, 0);
        assert_eq!(status.services.len(), 6);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_service_coordinator_subscribe() {
        let mut runtime = ActonApp::launch_async().await;
        let handle = ServiceCoordinatorAgent::spawn(&mut runtime).await.unwrap();

        let (request, rx) = Subscribe::new();
        handle.send(request).await;

        let subscriber = rx.await.expect("Failed to subscribe");
        assert!(subscriber.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_service_coordinator_health_check_updates_state() {
        let mut runtime = ActonApp::launch_async().await;
        let handle = ServiceCoordinatorAgent::spawn(&mut runtime).await.unwrap();

        // Subscribe to events
        let (subscribe_req, subscribe_rx) = Subscribe::new();
        handle.send(subscribe_req).await;
        let mut subscriber = subscribe_rx.await.expect("Failed to subscribe");

        // Report a healthy service
        handle
            .send(HealthCheckResult::success(ServiceId::Auth, 10))
            .await;

        // Wait a bit for processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Should receive status event (Unknown -> Healthy)
        let event = subscriber.try_recv();
        assert!(event.is_ok());
        let event = event.unwrap();
        assert_eq!(event.service_id, ServiceId::Auth);
        assert_eq!(event.new_state, ServiceState::Healthy);

        // Verify status
        let (status_req, status_rx) = GetServiceStatus::new();
        handle.send(status_req).await;
        let status = status_rx.await.expect("Failed to get status");
        assert_eq!(status.health_check_count, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_service_coordinator_circuit_breaker_integration() {
        let config = ServiceCoordinatorConfig::new().with_failure_threshold(2);
        let mut runtime = ActonApp::launch_async().await;
        let handle = ServiceCoordinatorAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Report failures to trigger circuit breaker
        handle
            .send(HealthCheckResult::failure(
                ServiceId::Data,
                "Error".to_string(),
            ))
            .await;
        handle
            .send(HealthCheckResult::failure(
                ServiceId::Data,
                "Error".to_string(),
            ))
            .await;

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify status shows unhealthy
        let (status_req, status_rx) = GetServiceStatus::new();
        handle.send(status_req).await;
        let status = status_rx.await.expect("Failed to get status");

        let (state, circuit) = status.services.get(&ServiceId::Data).unwrap();
        assert_eq!(*state, ServiceState::Unhealthy);
        assert_eq!(*circuit, CircuitState::Open);
    }
}

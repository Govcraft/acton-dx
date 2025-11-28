//! acton-reactive agents
//!
//! This module contains actor-based components for background processing,
//! session management, CSRF protection, and real-time features.

use acton_reactive::prelude::{AgentConfig, Ern};

pub mod csrf_manager;
pub mod hot_reload;
pub mod rate_limiter;
pub mod request_reply;
pub mod service_coordinator;
pub mod session_manager;

// Re-export public types for use by middleware and extractors
pub use csrf_manager::{
    CleanupExpired as CsrfCleanupExpired, CsrfManagerAgent, CsrfToken, DeleteToken,
    GetOrCreateToken, ValidateToken,
};
pub use hot_reload::{
    FileChanged, ForceReload, GetStats as HotReloadGetStats, HotReloadConfig,
    HotReloadCoordinatorAgent, HotReloadStats, ReloadEvent, ReloadType, Subscribe as HotReloadSubscribe,
    TriggerPendingReloads, UpdateConfig as HotReloadUpdateConfig,
};
pub use request_reply::{create_request_reply, send_response, ResponseChannel};
pub use rate_limiter::{
    CheckRateLimit, CleanupExpired as RateLimiterCleanupExpired, GetStats as RateLimiterGetStats,
    RateLimiterAgent, RateLimiterConfig, RateLimiterStats, RateLimitResult, ResetBucket, TokenBucket,
    UpdateConfig as RateLimiterUpdateConfig,
};
pub use service_coordinator::{
    CircuitBreaker, CircuitState, GetServiceStatus, HealthCheckResult, ServiceAvailable,
    ServiceCoordinatorAgent, ServiceCoordinatorConfig, ServiceHealth, ServiceId, ServiceState,
    ServiceStatusEvent, ServiceStatusResponse, ServiceUnavailable,
    Subscribe as ServiceCoordinatorSubscribe, UpdateConfig as ServiceCoordinatorUpdateConfig,
};
pub use session_manager::{
    // Unified messages (support both web handler and agent-to-agent patterns)
    AddFlash, CleanupExpired, DeleteSession, LoadSession, SaveSession, SessionManagerAgent,
    TakeFlashes,
};

/// Create a default agent configuration with the given name
///
/// This is a convenience function that creates an `AgentConfig` with:
/// - An ERN (Entity Resource Name) rooted at the given name
/// - No parent agent (standalone)
/// - No custom context
///
/// # Arguments
///
/// * `name` - The unique identifier for this agent type (e.g., "csrf_manager", "session_manager")
///
/// # Errors
///
/// Returns an error if the ERN cannot be created (invalid name format)
///
/// # Example
///
/// ```ignore
/// let config = default_agent_config("my_agent")?;
/// let builder = runtime.new_agent_with_config::<MyAgent>(config).await;
/// ```
pub fn default_agent_config(name: &str) -> anyhow::Result<AgentConfig> {
    AgentConfig::new(Ern::with_root(name)?, None, None)
}

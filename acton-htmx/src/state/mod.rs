//! Application state management
//!
//! Combines acton-service infrastructure with acton-reactive actors and
//! HTMX-specific components.

use crate::agents::{CsrfManagerAgent, SessionManagerAgent};
use crate::jobs::JobAgent;
use crate::oauth2::OAuth2Agent;
use crate::{config::ActonHtmxConfig, observability::ObservabilityConfig};
use acton_reactive::prelude::{AgentHandle, AgentRuntime};
use sqlx::PgPool;
use std::sync::Arc;

#[cfg(feature = "redis")]
use deadpool_redis::Pool as RedisPool;

/// Application state for acton-htmx applications
///
/// Combines:
/// - Configuration (from acton-service)
/// - Observability (from acton-service)
/// - Session management agent (from acton-reactive)
/// - CSRF protection agent (from acton-reactive)
/// - OAuth2 manager agent (from acton-reactive)
/// - Job processing agent (from acton-reactive)
/// - Database connection pool (PostgreSQL via SQLx)
/// - Redis cache (optional, for distributed sessions and job persistence)
///
/// Note: Askama compiles templates at build time, so no runtime template registry is needed.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::state::ActonHtmxState;
/// use acton_reactive::prelude::ActonApp;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     // Launch the Acton runtime - keep ownership in main
///     let mut runtime = ActonApp::launch();
///
///     // Create application state with agents
///     let state = ActonHtmxState::new(&mut runtime).await?;
///
///     // Build Axum router with state
///     let app = axum::Router::new()
///         .route("/", axum::routing::get(|| async { "Hello!" }))
///         .with_state(state);
///
///     // ... run server with graceful shutdown ...
///
///     // Shutdown the agent runtime after server stops
///     runtime.shutdown_all().await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ActonHtmxState {
    /// Application configuration
    config: Arc<ActonHtmxConfig>,

    /// Observability configuration
    observability: Arc<ObservabilityConfig>,

    /// Session manager agent handle
    ///
    /// Clone this freely - `AgentHandle` is designed for concurrent access
    session_manager: AgentHandle,

    /// CSRF manager agent handle
    ///
    /// Clone this freely - `AgentHandle` is designed for concurrent access
    csrf_manager: AgentHandle,

    /// OAuth2 manager agent handle
    ///
    /// Clone this freely - `AgentHandle` is designed for concurrent access
    oauth2_manager: AgentHandle,

    /// Job processing agent handle
    ///
    /// Clone this freely - `AgentHandle` is designed for concurrent access
    job_agent: AgentHandle,

    /// Database connection pool
    ///
    /// Shared across all requests for efficient connection management
    database_pool: Option<Arc<PgPool>>,

    /// Redis connection pool (optional)
    ///
    /// Used for distributed sessions and job persistence when enabled
    #[cfg(feature = "redis")]
    redis_pool: Option<RedisPool>,
}

impl ActonHtmxState {
    /// Create new application state with defaults
    ///
    /// Spawns the session manager agent with in-memory storage.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the Acton runtime. The caller retains
    ///   ownership for shutdown orchestration.
    ///
    /// # Errors
    ///
    /// Returns error if agent spawning fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::state::ActonHtmxState;
    /// use acton_reactive::prelude::ActonApp;
    ///
    /// let mut runtime = ActonApp::launch();
    /// let state = ActonHtmxState::new(&mut runtime).await?;
    /// ```
    pub async fn new(runtime: &mut AgentRuntime) -> anyhow::Result<Self> {
        let config = ActonHtmxConfig::default();
        let observability = ObservabilityConfig::default();
        let session_manager = SessionManagerAgent::spawn(runtime).await?;
        let csrf_manager = CsrfManagerAgent::spawn(runtime).await?;
        let oauth2_manager = OAuth2Agent::spawn(runtime).await?;
        let job_agent = JobAgent::spawn(runtime).await?;

        Ok(Self {
            config: Arc::new(config),
            observability: Arc::new(observability),
            session_manager,
            csrf_manager,
            oauth2_manager,
            job_agent,
            database_pool: None,
            #[cfg(feature = "redis")]
            redis_pool: None,
        })
    }

    /// Create application state with custom configuration
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the Acton runtime
    /// * `config` - Custom configuration for the application
    ///
    /// # Errors
    ///
    /// Returns error if agent spawning fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::{config::ActonHtmxConfig, state::ActonHtmxState};
    /// use acton_reactive::prelude::ActonApp;
    ///
    /// let mut runtime = ActonApp::launch();
    /// let config = ActonHtmxConfig::load_for_service("my-app")?;
    /// let state = ActonHtmxState::with_config(&mut runtime, config).await?;
    /// ```
    pub async fn with_config(
        runtime: &mut AgentRuntime,
        config: ActonHtmxConfig,
    ) -> anyhow::Result<Self> {
        let observability = ObservabilityConfig::new("acton-htmx");
        let session_manager = SessionManagerAgent::spawn(runtime).await?;
        let csrf_manager = CsrfManagerAgent::spawn(runtime).await?;
        let oauth2_manager = OAuth2Agent::spawn(runtime).await?;
        let job_agent = JobAgent::spawn(runtime).await?;

        Ok(Self {
            config: Arc::new(config),
            observability: Arc::new(observability),
            session_manager,
            csrf_manager,
            oauth2_manager,
            job_agent,
            database_pool: None,
            #[cfg(feature = "redis")]
            redis_pool: None,
        })
    }

    /// Get configuration reference
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::state::ActonHtmxState;
    ///
    /// let state = ActonHtmxState::new(&mut runtime).await?;
    /// let config = state.config();
    /// let timeout = config.htmx.request_timeout_ms;
    /// ```
    #[must_use]
    pub fn config(&self) -> &ActonHtmxConfig {
        &self.config
    }

    /// Get observability configuration
    #[must_use]
    pub fn observability(&self) -> &ObservabilityConfig {
        &self.observability
    }

    /// Get the session manager agent handle
    ///
    /// Use this to send session-related messages directly to the agent.
    /// For most use cases, prefer using the `SessionExtractor` in handlers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::agents::{LoadSessionRequest, SaveSessionRequest};
    ///
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     let (request, rx) = LoadSessionRequest::new(session_id);
    ///     state.session_manager().send(request).await;
    ///     let session_data = rx.await.ok().flatten();
    /// }
    /// ```
    #[must_use]
    pub const fn session_manager(&self) -> &AgentHandle {
        &self.session_manager
    }

    /// Get the CSRF manager agent handle
    ///
    /// Use this to send CSRF-related messages directly to the agent.
    /// For most use cases, prefer using the `CsrfMiddleware` and extractors.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::agents::{GetOrCreateTokenRequest, ValidateTokenRequest};
    ///
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     let (request, rx) = GetOrCreateTokenRequest::new(session_id);
    ///     state.csrf_manager().send(request).await;
    ///     let token = rx.await.ok();
    /// }
    /// ```
    #[must_use]
    pub const fn csrf_manager(&self) -> &AgentHandle {
        &self.csrf_manager
    }

    /// Get the OAuth2 manager agent handle
    ///
    /// Use this to send OAuth2-related messages directly to the agent.
    /// For most use cases, prefer using the OAuth2 handlers and extractors.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::oauth2::{GenerateState, ValidateState, OAuthProvider};
    ///
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     let oauth_state = state.oauth2_agent()
    ///         .ask(GenerateState { provider: OAuthProvider::Google })
    ///         .await?;
    /// }
    /// ```
    #[must_use]
    pub const fn oauth2_agent(&self) -> &AgentHandle {
        &self.oauth2_manager
    }

    /// Get the job processing agent handle
    ///
    /// Use this to send job-related messages directly to the agent.
    /// For most use cases, prefer using the job processing APIs in the `jobs` module.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::jobs::{EnqueueJob, JobId};
    ///
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     let job_handle = state.job_agent();
    ///     // Enqueue a job directly
    ///     job_handle.send(EnqueueJob { /* ... */ }).await;
    /// }
    /// ```
    #[must_use]
    pub const fn job_agent(&self) -> &AgentHandle {
        &self.job_agent
    }

    /// Get the database connection pool
    ///
    /// # Panics
    ///
    /// Panics if the database pool has not been initialized via `set_database_pool`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     let pool = state.database_pool();
    ///     let users = sqlx::query_as("SELECT * FROM users")
    ///         .fetch_all(pool)
    ///         .await?;
    /// }
    /// ```
    #[must_use]
    pub fn database_pool(&self) -> &PgPool {
        self.database_pool
            .as_ref()
            .expect("Database pool not initialized")
    }

    /// Set the database connection pool
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = PgPool::connect(&database_url).await?;
    /// state.set_database_pool(pool);
    /// ```
    pub fn set_database_pool(&mut self, pool: PgPool) {
        self.database_pool = Some(Arc::new(pool));
    }

    /// Get the Redis connection pool (if configured)
    ///
    /// Returns `None` if Redis is not enabled or not configured.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     if let Some(redis) = state.redis_pool() {
    ///         let mut conn = redis.get().await?;
    ///         // Use Redis connection
    ///     }
    /// }
    /// ```
    #[must_use]
    #[cfg(feature = "redis")]
    pub const fn redis_pool(&self) -> Option<&RedisPool> {
        self.redis_pool.as_ref()
    }

    /// Set the Redis connection pool
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let redis_pool = RedisPool::new(/* config */)?;
    /// state.set_redis_pool(redis_pool);
    /// ```
    #[cfg(feature = "redis")]
    pub fn set_redis_pool(&mut self, pool: RedisPool) {
        self.redis_pool = Some(pool);
    }

    // ========================================================================
    // Job Agent Helper Methods (Web Handler Pattern)
    // ========================================================================

    /// Get job metrics with timeout.
    ///
    /// Convenience method that handles the oneshot channel pattern for
    /// querying job metrics from the `JobAgent`.
    ///
    /// Uses a 100ms timeout to prevent handlers from hanging if the agent
    /// is slow or stopped.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Agent doesn't respond within timeout
    /// - Response channel is closed (agent stopped)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn handler(State(state): State<ActonHtmxState>) -> Result<Response> {
    ///     let metrics = state.get_job_metrics().await
    ///         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    ///
    ///     Ok(Json(metrics).into_response())
    /// }
    /// ```
    pub async fn get_job_metrics(&self) -> Result<crate::jobs::agent::JobMetrics, anyhow::Error> {
        use acton_reactive::prelude::AgentHandleInterface;
        use crate::jobs::agent::GetMetricsRequest;
        use std::time::Duration;

        let (request, rx) = GetMetricsRequest::new();
        self.job_agent().send(request).await;

        let timeout = Duration::from_millis(100);
        Ok(tokio::time::timeout(timeout, rx).await??)
    }

    /// Get job status with timeout.
    ///
    /// Convenience method that handles the oneshot channel pattern for
    /// querying job status from the `JobAgent`.
    ///
    /// Uses a 100ms timeout to prevent handlers from hanging if the agent
    /// is slow or stopped.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Agent doesn't respond within timeout
    /// - Response channel is closed (agent stopped)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::jobs::JobId;
    ///
    /// async fn handler(
    ///     State(state): State<ActonHtmxState>,
    ///     Path(job_id): Path<JobId>,
    /// ) -> Result<Response> {
    ///     let status = state.get_job_status(job_id).await
    ///         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    ///
    ///     match status {
    ///         Some(status) => Ok(Json(status).into_response()),
    ///         None => Err(StatusCode::NOT_FOUND),
    ///     }
    /// }
    /// ```
    pub async fn get_job_status(
        &self,
        id: crate::jobs::JobId,
    ) -> Result<Option<crate::jobs::JobStatus>, anyhow::Error> {
        use acton_reactive::prelude::AgentHandleInterface;
        use crate::jobs::agent::GetJobStatusRequest;
        use std::time::Duration;

        let (request, rx) = GetJobStatusRequest::new(id);
        self.job_agent().send(request).await;

        let timeout = Duration::from_millis(100);
        Ok(tokio::time::timeout(timeout, rx).await??)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acton_reactive::prelude::ActonApp;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_new_state() {
        let mut runtime = ActonApp::launch();
        let state = ActonHtmxState::new(&mut runtime)
            .await
            .expect("Failed to create state");
        assert_eq!(state.config().htmx.request_timeout_ms, 5000);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_with_config() {
        let mut runtime = ActonApp::launch();
        let mut config = ActonHtmxConfig::default();
        config.htmx.request_timeout_ms = 10000;

        let state = ActonHtmxState::with_config(&mut runtime, config)
            .await
            .expect("Failed to create state");

        assert_eq!(state.config().htmx.request_timeout_ms, 10000);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_clone_state() {
        let mut runtime = ActonApp::launch();
        let state = ActonHtmxState::new(&mut runtime)
            .await
            .expect("Failed to create state");
        let cloned = state.clone();

        // Both should reference the same Arc for config
        assert!(Arc::ptr_eq(&state.config, &cloned.config));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_manager_accessible() {
        let mut runtime = ActonApp::launch();
        let state = ActonHtmxState::new(&mut runtime)
            .await
            .expect("Failed to create state");

        // Should be able to get the session manager handle
        let _handle = state.session_manager();
    }
}

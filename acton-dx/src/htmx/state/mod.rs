//! Application state management
//!
//! Combines acton-service infrastructure with acton-reactive actors and
//! HTMX-specific components.

use crate::htmx::agents::{CsrfManagerAgent, SessionManagerAgent};
use crate::htmx::jobs::JobAgent;
use crate::htmx::oauth2::OAuth2Agent;
use crate::htmx::template::FrameworkTemplates;
use crate::htmx::{config::ActonHtmxConfig, observability::ObservabilityConfig};
use acton_reactive::prelude::{AgentHandle, AgentRuntime};
use std::sync::Arc;

#[cfg(feature = "postgres")]
use sqlx::PgPool;

#[cfg(feature = "sqlite")]
use sqlx::SqlitePool;

#[cfg(feature = "redis")]
use deadpool_redis::Pool as RedisPool;

#[cfg(feature = "microservices")]
use crate::htmx::clients::{ServiceRegistry, ServicesConfig};

/// Application state for acton-dx applications
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
/// - Framework templates (runtime-loadable HTML templates)
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

    /// PostgreSQL database connection pool
    ///
    /// Shared across all requests for efficient connection management
    #[cfg(feature = "postgres")]
    pg_pool: Option<Arc<PgPool>>,

    /// SQLite database connection pool
    ///
    /// Shared across all requests for efficient connection management
    #[cfg(feature = "sqlite")]
    sqlite_pool: Option<Arc<SqlitePool>>,

    /// Redis connection pool (optional)
    ///
    /// Used for distributed sessions and job persistence when enabled
    #[cfg(feature = "redis")]
    redis_pool: Option<RedisPool>,

    /// Framework templates for HTML rendering
    ///
    /// XDG-compliant template loader with hot reload support
    templates: FrameworkTemplates,

    /// Microservices registry for distributed service clients
    ///
    /// When enabled, provides access to auth, data, cedar, cache, email,
    /// and file services via gRPC clients.
    #[cfg(feature = "microservices")]
    services: Option<ServiceRegistry>,
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
        let templates = FrameworkTemplates::new()?;

        Ok(Self {
            config: Arc::new(config),
            observability: Arc::new(observability),
            session_manager,
            csrf_manager,
            oauth2_manager,
            job_agent,
            #[cfg(feature = "postgres")]
            pg_pool: None,
            #[cfg(feature = "sqlite")]
            sqlite_pool: None,
            #[cfg(feature = "redis")]
            redis_pool: None,
            templates,
            #[cfg(feature = "microservices")]
            services: None,
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
        let observability = ObservabilityConfig::new("acton-dx");
        let session_manager = SessionManagerAgent::spawn(runtime).await?;
        let csrf_manager = CsrfManagerAgent::spawn(runtime).await?;
        let oauth2_manager = OAuth2Agent::spawn(runtime).await?;
        let job_agent = JobAgent::spawn(runtime).await?;
        let templates = FrameworkTemplates::new()?;

        Ok(Self {
            config: Arc::new(config),
            observability: Arc::new(observability),
            session_manager,
            csrf_manager,
            oauth2_manager,
            job_agent,
            #[cfg(feature = "postgres")]
            pg_pool: None,
            #[cfg(feature = "sqlite")]
            sqlite_pool: None,
            #[cfg(feature = "redis")]
            redis_pool: None,
            templates,
            #[cfg(feature = "microservices")]
            services: None,
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

    /// Get framework templates
    ///
    /// Returns the XDG-compliant template loader for rendering framework HTML.
    /// Templates can be customized by placing files in `~/.config/acton-dx/templates/framework/`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn error_page(State(state): State<ActonHtmxState>) -> impl IntoResponse {
    ///     state.templates().render("errors/404.html", minijinja::context! {
    ///         message => "Page not found",
    ///         home_url => "/",
    ///     })
    /// }
    /// ```
    #[must_use]
    pub const fn templates(&self) -> &FrameworkTemplates {
        &self.templates
    }

    /// Get the session manager agent handle
    ///
    /// Use this to send session-related messages directly to the agent.
    /// For most use cases, prefer using the `SessionExtractor` in handlers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use acton_htmx::agents::{LoadSession, SaveSession};
    ///
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     let (request, rx) = LoadSession::with_response(session_id);
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
    /// use acton_htmx::agents::{GetOrCreateToken, ValidateToken};
    ///
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     let (request, rx) = GetOrCreateToken::new(session_id);
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

    /// Get the PostgreSQL database connection pool
    ///
    /// # Panics
    ///
    /// Panics if the PostgreSQL pool has not been initialized via `set_pg_pool`.
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
    #[cfg(feature = "postgres")]
    pub fn database_pool(&self) -> &PgPool {
        self.pg_pool
            .as_ref()
            .expect("PostgreSQL pool not initialized")
    }

    /// Get the SQLite database connection pool
    ///
    /// # Panics
    ///
    /// Panics if the SQLite pool has not been initialized via `set_sqlite_pool`.
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
    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    pub fn database_pool(&self) -> &SqlitePool {
        self.sqlite_pool
            .as_ref()
            .expect("SQLite pool not initialized")
    }

    /// Set the PostgreSQL database connection pool
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = PgPool::connect(&database_url).await?;
    /// state.set_database_pool(pool);
    /// ```
    #[cfg(feature = "postgres")]
    pub fn set_database_pool(&mut self, pool: PgPool) {
        self.pg_pool = Some(Arc::new(pool));
    }

    /// Set the SQLite database connection pool
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pool = SqlitePool::connect(&database_url).await?;
    /// state.set_database_pool(pool);
    /// ```
    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    pub fn set_database_pool(&mut self, pool: SqlitePool) {
        self.sqlite_pool = Some(Arc::new(pool));
    }

    /// Get the SQLite pool directly (when both postgres and sqlite are enabled)
    #[must_use]
    #[cfg(feature = "sqlite")]
    pub fn sqlite_pool(&self) -> Option<&SqlitePool> {
        self.sqlite_pool.as_deref()
    }

    /// Set the SQLite pool directly (when both postgres and sqlite are enabled)
    #[cfg(feature = "sqlite")]
    pub fn set_sqlite_pool(&mut self, pool: SqlitePool) {
        self.sqlite_pool = Some(Arc::new(pool));
    }

    /// Get the PostgreSQL pool directly (when both postgres and sqlite are enabled)
    #[must_use]
    #[cfg(feature = "postgres")]
    pub fn pg_pool(&self) -> Option<&PgPool> {
        self.pg_pool.as_deref()
    }

    /// Set the PostgreSQL pool directly (when both postgres and sqlite are enabled)
    #[cfg(feature = "postgres")]
    pub fn set_pg_pool(&mut self, pool: PgPool) {
        self.pg_pool = Some(Arc::new(pool));
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
    // Microservices Helper Methods
    // ========================================================================

    /// Get the microservices registry (if configured)
    ///
    /// Returns `None` if microservices are not configured.
    /// Use `set_services` or `set_services_from_config` to configure.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// async fn handler(State(state): State<ActonHtmxState>) {
    ///     if let Some(services) = state.services() {
    ///         let auth = services.auth()?;
    ///         let mut client = auth.write().await;
    ///         // Use auth client
    ///     }
    /// }
    /// ```
    #[must_use]
    #[cfg(feature = "microservices")]
    pub const fn services(&self) -> Option<&ServiceRegistry> {
        self.services.as_ref()
    }

    /// Set the microservices registry
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ServicesConfig {
    ///     auth_endpoint: Some("http://localhost:50051".to_string()),
    ///     ..Default::default()
    /// };
    /// let registry = ServiceRegistry::from_config(&config).await?;
    /// state.set_services(registry);
    /// ```
    #[cfg(feature = "microservices")]
    pub fn set_services(&mut self, registry: ServiceRegistry) {
        self.services = Some(registry);
    }

    /// Configure microservices from a config struct
    ///
    /// Convenience method that creates a `ServiceRegistry` from configuration.
    ///
    /// # Errors
    ///
    /// Returns error if any configured service fails to connect.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = ServicesConfig {
    ///     auth_endpoint: Some("http://localhost:50051".to_string()),
    ///     data_endpoint: Some("http://localhost:50052".to_string()),
    ///     ..Default::default()
    /// };
    /// state.set_services_from_config(&config).await?;
    /// ```
    #[cfg(feature = "microservices")]
    pub async fn set_services_from_config(
        &mut self,
        config: &ServicesConfig,
    ) -> Result<(), crate::htmx::clients::ClientError> {
        let registry = ServiceRegistry::from_config(config).await?;
        self.services = Some(registry);
        Ok(())
    }

    /// Check if the auth service is available
    ///
    /// Returns true if microservices are configured and auth service is connected.
    #[must_use]
    #[cfg(feature = "microservices")]
    pub fn has_auth_service(&self) -> bool {
        self.services.as_ref().is_some_and(ServiceRegistry::has_auth)
    }

    /// Check if the data service is available
    #[must_use]
    #[cfg(feature = "microservices")]
    pub fn has_data_service(&self) -> bool {
        self.services.as_ref().is_some_and(ServiceRegistry::has_data)
    }

    /// Check if the cedar service is available
    #[must_use]
    #[cfg(feature = "microservices")]
    pub fn has_cedar_service(&self) -> bool {
        self.services.as_ref().is_some_and(ServiceRegistry::has_cedar)
    }

    /// Check if the cache service is available
    #[must_use]
    #[cfg(feature = "microservices")]
    pub fn has_cache_service(&self) -> bool {
        self.services.as_ref().is_some_and(ServiceRegistry::has_cache)
    }

    /// Check if the email service is available
    #[must_use]
    #[cfg(feature = "microservices")]
    pub fn has_email_service(&self) -> bool {
        self.services.as_ref().is_some_and(ServiceRegistry::has_email)
    }

    /// Check if the file service is available
    #[must_use]
    #[cfg(feature = "microservices")]
    pub fn has_file_service(&self) -> bool {
        self.services.as_ref().is_some_and(ServiceRegistry::has_file)
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
    pub async fn get_job_metrics(&self) -> Result<super::jobs::agent::JobMetrics, anyhow::Error> {
        use acton_reactive::prelude::AgentHandleInterface;
        use super::jobs::agent::GetMetricsRequest;
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
        id: super::jobs::JobId,
    ) -> Result<Option<super::jobs::JobStatus>, anyhow::Error> {
        use acton_reactive::prelude::AgentHandleInterface;
        use super::jobs::agent::GetJobStatusRequest;
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

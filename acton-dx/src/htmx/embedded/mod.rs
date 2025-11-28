//! Embedded services runtime for single-binary deployments.
//!
//! This module provides the ability to run all microservices within a single process,
//! eliminating inter-process communication overhead while maintaining the same API.
//!
//! # Usage
//!
//! ```rust,no_run
//! use acton_dx::htmx::embedded::{EmbeddedServices, EmbeddedServicesConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = EmbeddedServicesConfig::default();
//! let services = EmbeddedServices::new(config);
//!
//! // Start all services in background
//! let handle = services.start().await?;
//!
//! // Get client connections (using loopback gRPC)
//! let registry = services.registry().await?;
//!
//! // When done, shut down cleanly
//! handle.shutdown().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! In embedded mode, all services run as tokio tasks within the same process:
//! - No process spawning overhead
//! - Shared memory for faster data transfer
//! - Single deployment binary
//! - Simplified configuration
//!
//! Communication still uses gRPC over localhost for API compatibility,
//! but without inter-process communication overhead.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// Configuration for embedded services.
#[derive(Debug, Clone)]
pub struct EmbeddedServicesConfig {
    /// Base port for services (services use sequential ports).
    pub base_port: u16,
    /// Host address to bind services to.
    pub host: String,
    /// Enable specific services (all enabled by default).
    pub enabled_services: HashMap<ServiceType, bool>,
}

impl Default for EmbeddedServicesConfig {
    fn default() -> Self {
        let mut enabled = HashMap::new();
        for service_type in ServiceType::all() {
            enabled.insert(*service_type, true);
        }

        Self {
            base_port: 50051,
            host: "127.0.0.1".to_string(),
            enabled_services,
        }
    }
}

impl EmbeddedServicesConfig {
    /// Create a new configuration with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base port for services.
    #[must_use]
    pub const fn with_base_port(mut self, port: u16) -> Self {
        self.base_port = port;
        self
    }

    /// Set the host address.
    #[must_use]
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Enable or disable a specific service.
    #[must_use]
    pub fn with_service(mut self, service: ServiceType, enabled: bool) -> Self {
        self.enabled_services.insert(service, enabled);
        self
    }

    /// Disable all services.
    #[must_use]
    pub fn disable_all(mut self) -> Self {
        for service_type in ServiceType::all() {
            self.enabled_services.insert(*service_type, false);
        }
        self
    }

    /// Enable only specific services.
    #[must_use]
    pub fn enable_only(mut self, services: &[ServiceType]) -> Self {
        // First disable all
        for service_type in ServiceType::all() {
            self.enabled_services.insert(*service_type, false);
        }
        // Then enable requested
        for service in services {
            self.enabled_services.insert(*service, true);
        }
        self
    }

    /// Check if a service is enabled.
    #[must_use]
    pub fn is_enabled(&self, service: ServiceType) -> bool {
        self.enabled_services.get(&service).copied().unwrap_or(true)
    }

    /// Get the port for a specific service.
    #[must_use]
    pub fn port_for(&self, service: ServiceType) -> u16 {
        self.base_port + service.port_offset()
    }

    /// Get the endpoint URL for a specific service.
    #[must_use]
    pub fn endpoint_for(&self, service: ServiceType) -> String {
        format!("http://{}:{}", self.host, self.port_for(service))
    }
}

/// Types of embedded services.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceType {
    /// Authentication service.
    Auth,
    /// Data service.
    Data,
    /// Cedar authorization service.
    Cedar,
    /// Cache service.
    Cache,
    /// Email service.
    Email,
    /// File service.
    File,
}

impl ServiceType {
    /// Get all service types.
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

    /// Get the port offset for this service type.
    #[must_use]
    pub const fn port_offset(&self) -> u16 {
        match self {
            Self::Auth => 0,
            Self::Data => 1,
            Self::Cedar => 2,
            Self::Cache => 3,
            Self::Email => 4,
            Self::File => 5,
        }
    }

    /// Get the display name for this service.
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
}

impl std::fmt::Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Handle to running embedded services.
pub struct EmbeddedServicesHandle {
    shutdown_tx: broadcast::Sender<()>,
    tasks: Vec<JoinHandle<()>>,
    config: EmbeddedServicesConfig,
}

impl EmbeddedServicesHandle {
    /// Shut down all services gracefully.
    ///
    /// # Errors
    ///
    /// Returns error if any service task panicked.
    pub async fn shutdown(self) -> Result<(), EmbeddedServicesError> {
        // Send shutdown signal to all services
        let _ = self.shutdown_tx.send(());

        // Wait for all tasks to complete
        for task in self.tasks {
            if let Err(e) = task.await {
                if e.is_panic() {
                    return Err(EmbeddedServicesError::TaskPanicked(e.to_string()));
                }
            }
        }

        Ok(())
    }

    /// Get the configuration used by these services.
    #[must_use]
    pub const fn config(&self) -> &EmbeddedServicesConfig {
        &self.config
    }

    /// Get the endpoint URL for a specific service.
    #[must_use]
    pub fn endpoint_for(&self, service: ServiceType) -> String {
        self.config.endpoint_for(service)
    }
}

/// Embedded services runtime.
///
/// Manages running all microservices within a single process.
#[derive(Debug, Clone)]
pub struct EmbeddedServices {
    config: Arc<EmbeddedServicesConfig>,
}

impl EmbeddedServices {
    /// Create a new embedded services runtime.
    #[must_use]
    pub fn new(config: EmbeddedServicesConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Start all enabled services.
    ///
    /// Services are started as tokio tasks and will run until shutdown.
    ///
    /// # Errors
    ///
    /// Returns error if any service fails to start.
    pub async fn start(&self) -> Result<EmbeddedServicesHandle, EmbeddedServicesError> {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let mut tasks = Vec::new();

        for service_type in ServiceType::all() {
            if !self.config.is_enabled(*service_type) {
                continue;
            }

            let addr: SocketAddr = format!(
                "{}:{}",
                self.config.host,
                self.config.port_for(*service_type)
            )
            .parse()
            .map_err(|e| EmbeddedServicesError::InvalidAddress(format!("{e}")))?;

            let shutdown_rx = shutdown_tx.subscribe();
            let task = self.spawn_service(*service_type, addr, shutdown_rx).await?;
            tasks.push(task);
        }

        Ok(EmbeddedServicesHandle {
            shutdown_tx,
            tasks,
            config: (*self.config).clone(),
        })
    }

    /// Spawn a single service task.
    async fn spawn_service(
        &self,
        service_type: ServiceType,
        addr: SocketAddr,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<JoinHandle<()>, EmbeddedServicesError> {
        // For now, we spawn placeholder tasks that wait for shutdown.
        // The actual service implementations would be added when the service
        // crates are made available as optional dependencies.
        //
        // In a full implementation, this would:
        // 1. Create the service implementation (e.g., AuthServiceImpl)
        // 2. Wrap it in a tonic Server
        // 3. Run the server with graceful shutdown

        let service_name = service_type.name().to_string();

        let task = tokio::spawn(async move {
            tracing::info!(
                service = %service_name,
                addr = %addr,
                "Embedded service started (placeholder)"
            );

            // Wait for shutdown signal
            let _ = shutdown_rx.recv().await;

            tracing::info!(
                service = %service_name,
                "Embedded service shutting down"
            );
        });

        Ok(task)
    }

    /// Create a services config for connecting to the embedded services.
    #[cfg(feature = "microservices")]
    #[must_use]
    pub fn services_config(&self) -> crate::htmx::clients::ServicesConfig {
        use crate::htmx::clients::ServicesConfig;

        ServicesConfig {
            auth_endpoint: self
                .config
                .is_enabled(ServiceType::Auth)
                .then(|| self.config.endpoint_for(ServiceType::Auth)),
            data_endpoint: self
                .config
                .is_enabled(ServiceType::Data)
                .then(|| self.config.endpoint_for(ServiceType::Data)),
            cedar_endpoint: self
                .config
                .is_enabled(ServiceType::Cedar)
                .then(|| self.config.endpoint_for(ServiceType::Cedar)),
            cache_endpoint: self
                .config
                .is_enabled(ServiceType::Cache)
                .then(|| self.config.endpoint_for(ServiceType::Cache)),
            email_endpoint: self
                .config
                .is_enabled(ServiceType::Email)
                .then(|| self.config.endpoint_for(ServiceType::Email)),
            file_endpoint: self
                .config
                .is_enabled(ServiceType::File)
                .then(|| self.config.endpoint_for(ServiceType::File)),
        }
    }
}

impl Default for EmbeddedServices {
    fn default() -> Self {
        Self::new(EmbeddedServicesConfig::default())
    }
}

/// Errors from embedded services.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddedServicesError {
    /// Invalid address format.
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// Service failed to start.
    #[error("Service failed to start: {0}")]
    StartFailed(String),

    /// A service task panicked.
    #[error("Service task panicked: {0}")]
    TaskPanicked(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EmbeddedServicesConfig::default();
        assert_eq!(config.base_port, 50051);
        assert_eq!(config.host, "127.0.0.1");

        // All services should be enabled by default
        for service_type in ServiceType::all() {
            assert!(config.is_enabled(*service_type));
        }
    }

    #[test]
    fn test_config_builder() {
        let config = EmbeddedServicesConfig::new()
            .with_base_port(60000)
            .with_host("0.0.0.0")
            .with_service(ServiceType::Auth, false);

        assert_eq!(config.base_port, 60000);
        assert_eq!(config.host, "0.0.0.0");
        assert!(!config.is_enabled(ServiceType::Auth));
        assert!(config.is_enabled(ServiceType::Data));
    }

    #[test]
    fn test_port_calculation() {
        let config = EmbeddedServicesConfig::new().with_base_port(50051);

        assert_eq!(config.port_for(ServiceType::Auth), 50051);
        assert_eq!(config.port_for(ServiceType::Data), 50052);
        assert_eq!(config.port_for(ServiceType::Cedar), 50053);
        assert_eq!(config.port_for(ServiceType::Cache), 50054);
        assert_eq!(config.port_for(ServiceType::Email), 50055);
        assert_eq!(config.port_for(ServiceType::File), 50056);
    }

    #[test]
    fn test_endpoint_generation() {
        let config = EmbeddedServicesConfig::new()
            .with_base_port(50051)
            .with_host("localhost");

        assert_eq!(
            config.endpoint_for(ServiceType::Auth),
            "http://localhost:50051"
        );
        assert_eq!(
            config.endpoint_for(ServiceType::File),
            "http://localhost:50056"
        );
    }

    #[test]
    fn test_enable_only() {
        let config = EmbeddedServicesConfig::new()
            .enable_only(&[ServiceType::Auth, ServiceType::Cache]);

        assert!(config.is_enabled(ServiceType::Auth));
        assert!(config.is_enabled(ServiceType::Cache));
        assert!(!config.is_enabled(ServiceType::Data));
        assert!(!config.is_enabled(ServiceType::Cedar));
        assert!(!config.is_enabled(ServiceType::Email));
        assert!(!config.is_enabled(ServiceType::File));
    }

    #[test]
    fn test_disable_all() {
        let config = EmbeddedServicesConfig::new().disable_all();

        for service_type in ServiceType::all() {
            assert!(!config.is_enabled(*service_type));
        }
    }

    #[test]
    fn test_service_type_display() {
        assert_eq!(format!("{}", ServiceType::Auth), "auth");
        assert_eq!(format!("{}", ServiceType::Data), "data");
    }

    #[tokio::test]
    async fn test_embedded_services_start_shutdown() {
        let services = EmbeddedServices::new(
            EmbeddedServicesConfig::new()
                .enable_only(&[ServiceType::Auth])
                .with_base_port(61000), // Use high port to avoid conflicts
        );

        let handle = services.start().await.unwrap();
        assert_eq!(handle.endpoint_for(ServiceType::Auth), "http://127.0.0.1:61000");

        // Shutdown should complete without errors
        handle.shutdown().await.unwrap();
    }
}

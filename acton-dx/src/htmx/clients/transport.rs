//! Transport abstraction for service communication.
//!
//! This module provides a transport-agnostic interface for communicating
//! with Acton DX microservices. It supports both IPC (Inter-Process Communication)
//! and gRPC transports, with IPC as the default for better performance
//! when services are co-located.
//!
//! # Transport Types
//!
//! - **IPC (default)**: Unix Domain Sockets for low-latency, local communication
//! - **gRPC**: HTTP/2-based protocol for distributed deployments
//!
//! # Configuration
//!
//! ```toml
//! [services]
//! transport = "ipc"  # or "grpc"
//!
//! [services.ipc]
//! socket_path = "/run/user/1000/acton/my-app/ipc.sock"  # optional, XDG default
//! timeout_ms = 30000
//! max_retries = 3
//!
//! [services.grpc]
//! auth_endpoint = "http://localhost:50051"
//! data_endpoint = "http://localhost:50052"
//! # ... other endpoints
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use acton_dx::htmx::clients::{TransportConfig, TransportType};
//!
//! // Create with IPC transport (default)
//! let config = TransportConfig::default();
//! assert_eq!(config.transport_type, TransportType::Ipc);
//!
//! // Create with gRPC transport
//! let config = TransportConfig::grpc();
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Type of transport to use for service communication.
///
/// IPC is the default and recommended transport for co-located services
/// (same host), providing lower latency than gRPC. gRPC should be used
/// for distributed deployments across different hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    /// Inter-Process Communication via Unix Domain Sockets (default).
    ///
    /// Best for:
    /// - Single-host deployments
    /// - Sidecar pattern
    /// - Lower latency requirements
    /// - Simpler configuration (no ports)
    #[default]
    Ipc,

    /// gRPC over HTTP/2.
    ///
    /// Best for:
    /// - Distributed deployments
    /// - Cross-host communication
    /// - Load balancing across multiple service instances
    /// - Existing gRPC infrastructure
    Grpc,
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ipc => write!(f, "ipc"),
            Self::Grpc => write!(f, "grpc"),
        }
    }
}

/// Configuration for IPC transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IpcTransportConfig {
    /// Optional custom socket path.
    ///
    /// If not set, uses XDG-compliant default:
    /// `$XDG_RUNTIME_DIR/acton/<app_name>/ipc.sock`
    pub socket_path: Option<PathBuf>,

    /// Application name for socket path generation.
    ///
    /// Used when `socket_path` is not explicitly set.
    pub app_name: String,

    /// Request timeout in milliseconds.
    pub timeout_ms: u64,

    /// Maximum number of connection retries.
    pub max_retries: u32,

    /// Delay between retries in milliseconds.
    pub retry_delay_ms: u64,

    /// Enable reconnection on connection loss.
    pub auto_reconnect: bool,

    /// Maximum message size in bytes.
    pub max_message_size: usize,
}

impl Default for IpcTransportConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            app_name: "acton-dx".to_string(),
            timeout_ms: 30_000,
            max_retries: 3,
            retry_delay_ms: 100,
            auto_reconnect: true,
            max_message_size: 1_048_576, // 1 MiB
        }
    }
}

impl IpcTransportConfig {
    /// Get the socket path to use.
    ///
    /// Returns the explicitly set socket path, or generates an XDG-compliant
    /// default path based on the app name.
    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        if let Some(ref path) = self.socket_path {
            return path.clone();
        }

        // XDG-compliant default: $XDG_RUNTIME_DIR/acton/<app_name>/ipc.sock
        if let Some(runtime_dir) = dirs::runtime_dir() {
            runtime_dir
                .join("acton")
                .join(&self.app_name)
                .join("ipc.sock")
        } else {
            // Fallback to /tmp if XDG_RUNTIME_DIR is not set
            PathBuf::from("/tmp")
                .join("acton")
                .join(&self.app_name)
                .join("ipc.sock")
        }
    }

    /// Create a new config with a specific socket path.
    #[must_use]
    pub fn with_socket_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.socket_path = Some(path.into());
        self
    }

    /// Create a new config with a specific app name.
    #[must_use]
    pub fn with_app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = name.into();
        self
    }

    /// Create a new config with a specific timeout.
    #[must_use]
    pub const fn with_timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = timeout;
        self
    }

    /// Create a new config with retry settings.
    #[must_use]
    pub const fn with_retries(mut self, max_retries: u32, delay_ms: u64) -> Self {
        self.max_retries = max_retries;
        self.retry_delay_ms = delay_ms;
        self
    }
}

/// Configuration for gRPC transport.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GrpcTransportConfig {
    /// Auth service endpoint (e.g., `"http://localhost:50051"`).
    pub auth_endpoint: Option<String>,

    /// Data service endpoint.
    pub data_endpoint: Option<String>,

    /// Cedar service endpoint.
    pub cedar_endpoint: Option<String>,

    /// Cache service endpoint.
    pub cache_endpoint: Option<String>,

    /// Email service endpoint.
    pub email_endpoint: Option<String>,

    /// File service endpoint.
    pub file_endpoint: Option<String>,

    /// Request timeout in milliseconds.
    pub timeout_ms: u64,

    /// Enable TLS for connections.
    pub tls_enabled: bool,
}

impl GrpcTransportConfig {
    /// Create a new gRPC config with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout_ms: 30_000,
            ..Default::default()
        }
    }

    /// Set the auth service endpoint.
    #[must_use]
    pub fn with_auth_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.auth_endpoint = Some(endpoint.into());
        self
    }

    /// Set the data service endpoint.
    #[must_use]
    pub fn with_data_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.data_endpoint = Some(endpoint.into());
        self
    }

    /// Set the cedar service endpoint.
    #[must_use]
    pub fn with_cedar_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.cedar_endpoint = Some(endpoint.into());
        self
    }

    /// Set the cache service endpoint.
    #[must_use]
    pub fn with_cache_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.cache_endpoint = Some(endpoint.into());
        self
    }

    /// Set the email service endpoint.
    #[must_use]
    pub fn with_email_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.email_endpoint = Some(endpoint.into());
        self
    }

    /// Set the file service endpoint.
    #[must_use]
    pub fn with_file_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.file_endpoint = Some(endpoint.into());
        self
    }

    /// Set all endpoints to localhost with sequential ports.
    ///
    /// Useful for local development with embedded services.
    #[must_use]
    pub fn localhost(base_port: u16) -> Self {
        Self {
            auth_endpoint: Some(format!("http://localhost:{}", base_port)),
            data_endpoint: Some(format!("http://localhost:{}", base_port + 1)),
            cedar_endpoint: Some(format!("http://localhost:{}", base_port + 2)),
            cache_endpoint: Some(format!("http://localhost:{}", base_port + 3)),
            email_endpoint: Some(format!("http://localhost:{}", base_port + 4)),
            file_endpoint: Some(format!("http://localhost:{}", base_port + 5)),
            timeout_ms: 30_000,
            tls_enabled: false,
        }
    }
}

/// Transport configuration for service communication.
///
/// Determines which transport mechanism is used to communicate
/// with Acton DX microservices.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TransportConfig {
    /// The transport type to use (default: IPC).
    pub transport_type: TransportType,

    /// IPC transport configuration.
    pub ipc: IpcTransportConfig,

    /// gRPC transport configuration.
    pub grpc: GrpcTransportConfig,

    /// Fallback behavior when primary transport fails.
    pub fallback: FallbackConfig,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: TransportType::Ipc, // IPC is the default
            ipc: IpcTransportConfig::default(),
            grpc: GrpcTransportConfig::default(),
            fallback: FallbackConfig::default(),
        }
    }
}

impl TransportConfig {
    /// Create a new transport config with IPC as the transport type.
    #[must_use]
    pub fn ipc() -> Self {
        Self {
            transport_type: TransportType::Ipc,
            ..Default::default()
        }
    }

    /// Create a new transport config with gRPC as the transport type.
    #[must_use]
    pub fn grpc() -> Self {
        Self {
            transport_type: TransportType::Grpc,
            ..Default::default()
        }
    }

    /// Set the transport type.
    #[must_use]
    pub const fn with_transport_type(mut self, transport_type: TransportType) -> Self {
        self.transport_type = transport_type;
        self
    }

    /// Configure IPC transport settings.
    #[must_use]
    pub fn with_ipc_config(mut self, config: IpcTransportConfig) -> Self {
        self.ipc = config;
        self
    }

    /// Configure gRPC transport settings.
    #[must_use]
    pub fn with_grpc_config(mut self, config: GrpcTransportConfig) -> Self {
        self.grpc = config;
        self
    }

    /// Configure fallback behavior.
    #[must_use]
    pub fn with_fallback(mut self, fallback: FallbackConfig) -> Self {
        self.fallback = fallback;
        self
    }

    /// Check if IPC transport is selected.
    #[must_use]
    pub const fn is_ipc(&self) -> bool {
        matches!(self.transport_type, TransportType::Ipc)
    }

    /// Check if gRPC transport is selected.
    #[must_use]
    pub const fn is_grpc(&self) -> bool {
        matches!(self.transport_type, TransportType::Grpc)
    }
}

/// Fallback configuration when primary transport fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FallbackConfig {
    /// Enable fallback to alternative transport.
    pub enabled: bool,

    /// The fallback transport type (opposite of primary).
    /// If primary is IPC, fallback is gRPC and vice versa.
    pub fallback_type: Option<TransportType>,

    /// Maximum time to wait before trying fallback (milliseconds).
    pub fallback_timeout_ms: u64,

    /// Log fallback events.
    pub log_fallback: bool,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            fallback_type: None,
            fallback_timeout_ms: 1_000,
            log_fallback: true,
        }
    }
}

impl FallbackConfig {
    /// Enable fallback with automatic type detection.
    ///
    /// When primary is IPC, fallback will be gRPC and vice versa.
    #[must_use]
    pub fn enable_auto(primary: TransportType) -> Self {
        let fallback_type = match primary {
            TransportType::Ipc => TransportType::Grpc,
            TransportType::Grpc => TransportType::Ipc,
        };

        Self {
            enabled: true,
            fallback_type: Some(fallback_type),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_transport_is_ipc() {
        let config = TransportConfig::default();
        assert_eq!(config.transport_type, TransportType::Ipc);
        assert!(config.is_ipc());
        assert!(!config.is_grpc());
    }

    #[test]
    fn test_grpc_transport_config() {
        let config = TransportConfig::grpc();
        assert_eq!(config.transport_type, TransportType::Grpc);
        assert!(config.is_grpc());
        assert!(!config.is_ipc());
    }

    #[test]
    fn test_ipc_socket_path_default() {
        let config = IpcTransportConfig::default();
        let path = config.socket_path();

        // Should contain acton and app name
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("acton"));
        assert!(path_str.contains("acton-dx"));
        assert!(path_str.ends_with("ipc.sock"));
    }

    #[test]
    fn test_ipc_socket_path_custom() {
        let config = IpcTransportConfig::default()
            .with_socket_path("/custom/path/service.sock");
        let path = config.socket_path();
        assert_eq!(path, PathBuf::from("/custom/path/service.sock"));
    }

    #[test]
    fn test_grpc_localhost_config() {
        let config = GrpcTransportConfig::localhost(50051);

        assert_eq!(
            config.auth_endpoint,
            Some("http://localhost:50051".to_string())
        );
        assert_eq!(
            config.data_endpoint,
            Some("http://localhost:50052".to_string())
        );
        assert_eq!(
            config.file_endpoint,
            Some("http://localhost:50056".to_string())
        );
    }

    #[test]
    fn test_transport_type_display() {
        assert_eq!(format!("{}", TransportType::Ipc), "ipc");
        assert_eq!(format!("{}", TransportType::Grpc), "grpc");
    }

    #[test]
    fn test_fallback_auto() {
        let fallback = FallbackConfig::enable_auto(TransportType::Ipc);
        assert!(fallback.enabled);
        assert_eq!(fallback.fallback_type, Some(TransportType::Grpc));

        let fallback = FallbackConfig::enable_auto(TransportType::Grpc);
        assert_eq!(fallback.fallback_type, Some(TransportType::Ipc));
    }

    #[test]
    fn test_transport_config_builder() {
        let config = TransportConfig::ipc()
            .with_ipc_config(
                IpcTransportConfig::default()
                    .with_app_name("my-service")
                    .with_timeout_ms(60_000),
            )
            .with_fallback(FallbackConfig::enable_auto(TransportType::Ipc));

        assert!(config.is_ipc());
        assert_eq!(config.ipc.app_name, "my-service");
        assert_eq!(config.ipc.timeout_ms, 60_000);
        assert!(config.fallback.enabled);
    }
}

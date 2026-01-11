//! Service clients for microservices communication.
//!
//! This module provides client wrappers for communicating with Acton DX microservices
//! using either IPC (Unix Domain Sockets) or gRPC transports.
//!
//! ## Transport Types
//!
//! - **IPC (default)**: Low-latency Unix Domain Socket communication for co-located services
//! - **gRPC**: HTTP/2-based protocol for distributed deployments
//!
//! ## Available Clients
//!
//! - [`AuthClient`] - Authentication, sessions, passwords, CSRF tokens, and users (gRPC)
//! - [`DataClient`] - Database queries, transactions, and migrations (gRPC)
//! - [`CedarClient`] - Cedar-based authorization (gRPC)
//! - [`CacheClient`] - Redis caching and rate limiting (gRPC)
//! - [`EmailClient`] - Email sending (gRPC)
//! - [`FileClient`] - File storage and retrieval (gRPC)
//!
//! ## IPC Clients
//!
//! - [`ipc::IpcAuthClient`] - Auth operations over IPC
//! - [`ipc::IpcClient`] - Generic IPC client for custom operations
//!
//! # Configuration
//!
//! ```toml
//! [services]
//! transport = "ipc"  # or "grpc" (ipc is the default)
//!
//! [services.ipc]
//! app_name = "my-app"
//! timeout_ms = 30000
//!
//! [services.grpc]
//! auth_endpoint = "http://localhost:50051"
//! data_endpoint = "http://localhost:50052"
//! ```
//!
//! # Usage - gRPC (traditional)
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use acton_dx::htmx::clients::AuthClient;
//!
//! let mut auth = AuthClient::connect("http://localhost:50051").await?;
//!
//! // Create a session
//! let session = auth.create_session(None, 3600, Default::default()).await?;
//! println!("Session ID: {}", session.session_id);
//! # Ok(())
//! # }
//! ```
//!
//! # Usage - IPC (recommended for sidecars)
//!
//! ```rust,ignore
//! use acton_dx::htmx::clients::ipc::{IpcAuthClient, IpcClientConfig};
//!
//! let config = IpcClientConfig::default();
//! let auth = IpcAuthClient::connect(config).await?;
//!
//! // Create a session over IPC
//! let session = auth.create_session(None, 3600, Default::default()).await?;
//! println!("Session ID: {}", session.session_id);
//! ```
//!
//! # Service Registry
//!
//! For applications that need multiple services, use [`ServiceRegistry`] to
//! manage all connections:
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use acton_dx::htmx::clients::{ServiceRegistry, ServicesConfig};
//!
//! let config = ServicesConfig {
//!     auth_endpoint: Some("http://localhost:50051".to_string()),
//!     data_endpoint: Some("http://localhost:50052".to_string()),
//!     ..Default::default()
//! };
//!
//! let registry = ServiceRegistry::from_config(&config).await?;
//!
//! // Access the auth client
//! let auth = registry.auth()?;
//! # Ok(())
//! # }
//! ```

mod auth;
mod cache;
mod cedar;
mod data;
mod email;
mod error;
mod file;
pub mod ipc;
mod registry;
pub mod transport;

pub use auth::AuthClient;
pub use cache::{CacheClient, RateLimitResult};
pub use cedar::{AuthorizationRequest, AuthorizationResult, CedarClient, ReloadResult, ValidationResult};
pub use data::{DataClient, ExecuteResult, MigrationResult, PingResult};
pub use email::{BatchSendResult, EmailAddr, EmailAttachment, EmailClient, EmailMessage, SendResult};
pub use error::ClientError;
pub use file::{DownloadResult, FileClient, ListResult, SignedUrlResult, StoredFileInfo, UploadResult};
pub use registry::{ServiceRegistry, ServicesConfig};
pub use transport::{
    FallbackConfig, GrpcTransportConfig, IpcTransportConfig, TransportConfig, TransportType,
};

// Re-export proto types that might be useful for users
pub use acton_dx_proto::auth::v1::{FlashMessage, Session, User};
pub use acton_dx_proto::data::v1::{MigrationInfo, Row, Value};

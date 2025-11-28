//! Service clients for microservices communication.
//!
//! This module provides gRPC client wrappers for all Acton DX microservices:
//!
//! - [`AuthClient`] - Authentication, sessions, passwords, CSRF tokens, and users
//! - [`DataClient`] - Database queries, transactions, and migrations
//! - [`CedarClient`] - Cedar-based authorization
//! - [`CacheClient`] - Redis caching and rate limiting
//! - [`EmailClient`] - Email sending
//! - [`FileClient`] - File storage and retrieval
//!
//! # Usage
//!
//! Each client can be created by connecting to its corresponding service:
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
mod registry;

pub use auth::AuthClient;
pub use cache::{CacheClient, RateLimitResult};
pub use cedar::{AuthorizationRequest, AuthorizationResult, CedarClient, ReloadResult, ValidationResult};
pub use data::{DataClient, ExecuteResult, MigrationResult, PingResult};
pub use email::{BatchSendResult, EmailAddr, EmailAttachment, EmailClient, EmailMessage, SendResult};
pub use error::ClientError;
pub use file::{DownloadResult, FileClient, ListResult, SignedUrlResult, StoredFileInfo, UploadResult};
pub use registry::{ServiceRegistry, ServicesConfig};

// Re-export proto types that might be useful for users
pub use acton_dx_proto::auth::v1::{FlashMessage, Session, User};
pub use acton_dx_proto::data::v1::{MigrationInfo, Row, Value};

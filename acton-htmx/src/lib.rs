//! acton-htmx: Opinionated Rust web framework for HTMX applications
//!
//! This framework builds on battle-tested components from the Acton ecosystem:
//! - **acton-service**: Configuration, observability, middleware, connection pools
//! - **acton-reactive**: Actor-based background jobs, sessions, and real-time features
//!
//! # Design Principles
//!
//! 1. **Convention Over Configuration**: Smart defaults everywhere
//! 2. **Security by Default**: CSRF, secure sessions, security headers enabled
//! 3. **HTMX-First Architecture**: Response helpers and patterns for hypermedia
//! 4. **Type Safety Without Ceremony**: Compile-time guarantees via Rust's type system
//! 5. **Idiomatic Excellence**: Generated code exemplifies Rust best practices
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use acton_htmx::prelude::*;
//! use acton_reactive::prelude::ActonApp;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Launch the Acton runtime
//!     let mut runtime = ActonApp::launch();
//!
//!     // Initialize application state (spawns session manager agent)
//!     let state = ActonHtmxState::new(&mut runtime).await?;
//!
//!     // Build router with HTMX handlers and session middleware
//!     let app = axum::Router::new()
//!         .route("/", axum::routing::get(index))
//!         .layer(SessionLayer::new(&state))
//!         .with_state(state);
//!
//!     // Start server
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
//!     axum::serve(listener, app).await?;
//!
//!     // Shutdown the agent runtime
//!     runtime.shutdown_all().await?;
//!
//!     Ok(())
//! }
//!
//! async fn index() -> &'static str {
//!     "Hello, HTMX!"
//! }
//! ```
//!
//! # Features
//!
//! - `postgres` - `PostgreSQL` database support (default)
//! - `sqlite` - `SQLite` database support
//! - `mysql` - `MySQL` database support
//! - `redis` - Redis session and cache support (default)
//! - `otel-metrics` - OpenTelemetry metrics collection
//!
//! # Architecture
//!
//! See the [architecture overview](../../../.claude/architecture-overview.md) for details.

// Lint configuration is handled at the workspace level in Cargo.toml

// Public modules (exported in public API)
pub mod auth;
pub mod config;
pub mod email;
pub mod error;
pub mod extractors;
pub mod forms;
pub mod handlers;
pub mod health;
pub mod htmx;
pub mod jobs;
pub mod oauth2;
pub mod observability;
pub mod state;
pub mod storage;
pub mod template;

// Public modules for actors and agents
pub mod agents;

// Public middleware module (needed for session layer)
pub mod middleware;

// Testing utilities module (available in test builds)
#[cfg(test)]
pub mod testing;

pub mod prelude {
    //! Convenience re-exports for common types and traits
    //!
    //! Commonly used types and traits for building applications
    //!
    //! # Examples
    //!
    //! ```rust
    //! use acton_htmx::prelude::*;
    //! ```

    // HTMX extractors and responders (from axum-htmx)
    pub use crate::htmx::{
        // Middleware
        AutoVaryLayer,
        // Request extractors
        HxBoosted,
        HxCurrentUrl,
        HxHistoryRestoreRequest,
        // Response helpers
        HxLocation,
        HxPrompt,
        HxPushUrl,
        HxRedirect,
        HxRefresh,
        HxReplaceUrl,
        HxRequest,
        HxRequestGuardLayer,
        HxReselect,
        HxResponseTrigger,
        HxReswap,
        HxRetarget,
        HxTarget,
        HxTrigger,
        HxTriggerName,
        // acton-htmx extensions
        HxSwapOob,
        SwapStrategy,
    };

    // Template traits
    pub use crate::template::{HxTemplate, TemplateRegistry};

    // Form handling
    pub use crate::forms::{
        FieldBuilder, FieldError, FormBuilder, FormField, FormRenderOptions, FormRenderer,
        InputType, SelectOption, ValidationErrors,
    };

    // Authentication extractors
    pub use crate::auth::{Authenticated, OptionalAuth, Session};

    // Extractors
    pub use crate::extractors::{FileUpload, FileUploadError, FlashExtractor, MultiFileUpload, SessionExtractor};

    // Storage
    pub use crate::storage::{FileStorage, LocalFileStorage, StorageError, StoredFile, UploadedFile};

    // Error types
    pub use crate::error::ActonHtmxError;

    // Application state
    pub use crate::state::ActonHtmxState;

    // Session middleware
    pub use crate::middleware::{SessionConfig, SessionLayer};

    // Background jobs
    pub use crate::jobs::{Job, JobAgent, JobError, JobId, JobResult, JobStatus};

    // Email system
    pub use crate::email::{
        AwsSesBackend, ConsoleBackend, Email, EmailError, EmailSender, EmailTemplate,
        SendEmailJob, SimpleEmailTemplate, SmtpBackend,
    };

    // OAuth2 authentication
    pub use crate::oauth2::{
        GitHubProvider, GoogleProvider, OAuthAccount, OAuthConfig, OAuthError, OAuthProvider,
        OAuthState, OAuthToken, OAuthUserInfo, OidcProvider, ProviderConfig,
        OAuth2Agent, initiate_oauth, handle_oauth_callback, unlink_oauth_account,
    };

    // Re-export key dependencies for framework users
    // These allow users to avoid adding these crates to their Cargo.toml
    pub use acton_reactive;
    pub use anyhow;
    pub use askama;
    pub use axum;
    pub use serde;
    pub use serde_json;
    pub use sqlx;
    pub use thiserror;
    pub use tokio;
    pub use tower;
    pub use tower_http;
    pub use tracing;
    pub use tracing_subscriber;
    pub use validator;

    // Convenience for JSON responses
    pub use serde_json::json;
}

//! HTMX web framework module
//!
//! This module contains all the HTMX web framework functionality:
//! - Authentication and sessions
//! - Form handling with validation
//! - HTMX response types
//! - Middleware (CSRF, sessions, security headers)
//! - Email sending
//! - File storage
//! - Background jobs
//! - OAuth2 authentication
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use acton_dx::htmx::prelude::*;
//! use acton_reactive::prelude::ActonApp;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Launch the Acton runtime
//!     let mut runtime = ActonApp::launch_async().await;
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

// Public modules
pub mod agents;
pub mod auth;
pub mod config;
pub mod email;
pub mod error;
pub mod extractors;
pub mod forms;
pub mod handlers;
pub mod health;
pub mod jobs;
pub mod middleware;
pub mod oauth2;
pub mod observability;
pub mod responses;
pub mod state;
pub mod storage;
pub mod template;

// Microservices clients (available with microservices feature)
#[cfg(feature = "microservices")]
pub mod clients;

// Embedded services runtime (available with microservices feature)
#[cfg(feature = "microservices")]
pub mod embedded;

// Testing utilities module (available in test builds)
#[cfg(test)]
pub mod testing;

pub mod prelude {
    //! Convenience re-exports for common types and traits
    //!
    //! Commonly used types and traits for building HTMX applications
    //!
    //! # Examples
    //!
    //! ```rust
    //! use acton_dx::htmx::prelude::*;
    //! ```

    // HTMX extractors and responders (from axum-htmx via responses module)
    pub use super::responses::{
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
        // acton-dx extensions
        HxSwapOob,
        SwapStrategy,
    };

    // Template traits
    pub use super::template::{HxTemplate, TemplateRegistry};

    // Form handling
    pub use super::forms::{
        FieldBuilder, FieldError, FormBuilder, FormField, FormRenderOptions, FormRenderer,
        InputType, SelectOption, ValidationErrors,
    };

    // Authentication extractors
    pub use super::auth::{Authenticated, OptionalAuth, Session};

    // Extractors
    pub use super::extractors::{
        FileUpload, FileUploadError, FlashExtractor, MultiFileUpload, SessionExtractor,
    };

    // Storage
    pub use super::storage::{
        FileStorage, LocalFileStorage, StorageError, StoredFile, UploadedFile,
    };

    // Error types
    pub use super::error::ActonHtmxError;

    // Application state
    pub use super::state::ActonHtmxState;

    // Session middleware
    pub use super::middleware::{SessionConfig, SessionLayer};

    // Background jobs
    pub use super::jobs::{Job, JobAgent, JobError, JobId, JobResult, JobStatus};

    // Email system
    pub use super::email::{
        AwsSesBackend, ConsoleBackend, Email, EmailError, EmailSender, EmailTemplate,
        SendEmailJob, SimpleEmailTemplate, SmtpBackend,
    };

    // OAuth2 authentication (postgres-independent types)
    pub use super::oauth2::{
        GitHubProvider, GoogleProvider, OAuth2Agent, OAuthConfig, OAuthError, OAuthProvider,
        OAuthState, OAuthToken, OAuthUserInfo, OidcProvider, ProviderConfig,
    };

    // OAuth2 handlers and models (postgres only)
    #[cfg(feature = "postgres")]
    pub use super::oauth2::{
        handle_oauth_callback, initiate_oauth, unlink_oauth_account, OAuthAccount,
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

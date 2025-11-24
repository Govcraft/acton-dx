//! OAuth2 authentication module
//!
//! This module provides OAuth2 authentication support for acton-htmx applications,
//! including:
//! - Google OAuth2 (with OpenID Connect)
//! - GitHub OAuth2
//! - Generic OpenID Connect provider
//!
//! # Features
//!
//! - **CSRF Protection**: State tokens prevent cross-site request forgery attacks
//! - **PKCE Support**: Proof Key for Code Exchange for enhanced security
//! - **Account Linking**: Link multiple OAuth providers to a single user account
//! - **Type Safety**: Strongly typed providers and configurations
//! - **acton-reactive Integration**: State management via agents
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use acton_htmx::oauth2::{OAuthConfig, ProviderConfig};
//! use axum::{Router, routing::get};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure OAuth2 providers
//! let oauth_config = OAuthConfig {
//!     google: Some(ProviderConfig {
//!         client_id: std::env::var("GOOGLE_CLIENT_ID")?,
//!         client_secret: std::env::var("GOOGLE_CLIENT_SECRET")?,
//!         redirect_uri: "http://localhost:3000/auth/google/callback".to_string(),
//!         scopes: vec!["openid".to_string(), "email".to_string(), "profile".to_string()],
//!         auth_url: None,
//!         token_url: None,
//!         userinfo_url: None,
//!     }),
//!     github: Some(ProviderConfig {
//!         client_id: std::env::var("GITHUB_CLIENT_ID")?,
//!         client_secret: std::env::var("GITHUB_CLIENT_SECRET")?,
//!         redirect_uri: "http://localhost:3000/auth/github/callback".to_string(),
//!         scopes: vec!["read:user".to_string(), "user:email".to_string()],
//!         auth_url: None,
//!         token_url: None,
//!         userinfo_url: None,
//!     }),
//!     oidc: None,
//! };
//!
//! // Add OAuth2 routes to your router
//! let app = Router::new()
//!     .route("/auth/:provider", get(acton_htmx::oauth2::handlers::initiate_oauth))
//!     .route("/auth/:provider/callback", get(acton_htmx::oauth2::handlers::handle_oauth_callback))
//!     .route("/auth/:provider/unlink", get(acton_htmx::oauth2::handlers::unlink_oauth_account));
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration
//!
//! OAuth2 providers are configured via the `OAuthConfig` struct, which should be
//! stored in your application configuration:
//!
//! ```toml
//! [oauth2.google]
//! client_id = "your-google-client-id"
//! client_secret = "your-google-client-secret"
//! redirect_uri = "http://localhost:3000/auth/google/callback"
//! scopes = ["openid", "email", "profile"]
//!
//! [oauth2.github]
//! client_id = "your-github-client-id"
//! client_secret = "your-github-client-secret"
//! redirect_uri = "http://localhost:3000/auth/github/callback"
//! scopes = ["read:user", "user:email"]
//! ```
//!
//! # Security Considerations
//!
//! - **State Tokens**: CSRF state tokens are generated using cryptographically secure random
//!   number generators and expire after 10 minutes
//! - **PKCE**: All providers use PKCE (Proof Key for Code Exchange) to prevent authorization
//!   code interception attacks
//! - **State Validation**: State tokens are validated server-side using the OAuth2Agent
//! - **One-Time Use**: State tokens are removed after successful validation
//! - **Session Storage**: PKCE verifiers are stored in secure HTTP-only session cookies
//!
//! # Database Schema
//!
//! The OAuth2 module requires the `oauth_accounts` table (see migration 002):
//!
//! ```sql
//! CREATE TABLE oauth_accounts (
//!     id BIGSERIAL PRIMARY KEY,
//!     user_id BIGINT NOT NULL,
//!     provider TEXT NOT NULL CHECK (provider IN ('google', 'github', 'oidc')),
//!     provider_user_id TEXT NOT NULL,
//!     email TEXT NOT NULL,
//!     name TEXT,
//!     avatar_url TEXT,
//!     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
//!     CONSTRAINT fk_oauth_accounts_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
//!     CONSTRAINT unique_oauth_account UNIQUE (provider, provider_user_id)
//! );
//! ```

pub mod agent;
pub mod handlers;
pub mod http;
pub mod models;
pub mod providers;
pub mod types;

pub use agent::{OAuth2Agent, GenerateState, ValidateState, RemoveState, CleanupExpired};
pub use handlers::{initiate_oauth, handle_oauth_callback, unlink_oauth_account};
pub use models::OAuthAccount;
pub use providers::{GitHubProvider, GoogleProvider, OidcProvider};
pub use types::{
    OAuthConfig, OAuthError, OAuthProvider, OAuthState, OAuthToken, OAuthUserInfo, ProviderConfig,
};

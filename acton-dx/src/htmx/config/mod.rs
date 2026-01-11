//! Configuration management for acton-dx
//!
//! Extends acton-service's XDG-compliant configuration system with HTMX-specific
//! settings. Configuration is loaded from multiple sources with clear precedence:
//!
//! 1. Environment variables (highest priority, `ACTON_` prefix)
//! 2. `./config.toml` (development)
//! 3. `~/.config/acton-service/{service_name}/config.toml` (user config, XDG)
//! 4. `/etc/acton-service/{service_name}/config.toml` (system config)
//! 5. Hardcoded defaults (fallback)
//!
//! # Example Configuration
//!
//! ```toml
//! # config.toml
//! [service]
//! name = "my-htmx-app"
//! port = 3000
//!
//! [database]
//! url = "postgres://localhost/myapp"
//! optional = true
//! lazy_init = true
//!
//! # HTMX-specific settings (flattened custom config)
//! [htmx]
//! request_timeout_ms = 5000
//! history_enabled = true
//! auto_vary = true
//!
//! [templates]
//! template_dir = "./templates"
//! cache_enabled = true
//! hot_reload = true
//!
//! [security]
//! csrf_enabled = true
//! session_max_age_secs = 86400
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use acton_dx::htmx::config::ActonHtmxConfig;
//! use acton_service::prelude::Config;
//!
//! // Load configuration using acton-service's Config system
//! let config: Config<ActonHtmxConfig> = Config::load_for_service("my-app")?;
//!
//! // Access framework config (from acton-service)
//! let port = config.service.port;
//! let db_url = config.database_url();
//!
//! // Access HTMX-specific config (flattened)
//! let timeout = config.custom.htmx.request_timeout_ms;
//! let csrf_enabled = config.custom.security.csrf_enabled;
//! ```

// Import serde through acton-service prelude
use acton_service::prelude::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::htmx::oauth2::types::OAuthConfig;

/// HTMX-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmxSettings {
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,

    /// Enable HTMX history support
    pub history_enabled: bool,

    /// Enable auto-vary middleware for caching
    pub auto_vary: bool,

    /// Enable request guards for HTMX-only routes
    pub guards_enabled: bool,
}

impl Default for HtmxSettings {
    fn default() -> Self {
        Self {
            request_timeout_ms: 5000,
            history_enabled: true,
            auto_vary: true,
            guards_enabled: false,
        }
    }
}

/// Template engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplateSettings {
    /// Directory containing Askama templates
    pub template_dir: PathBuf,

    /// Enable template caching
    pub cache_enabled: bool,

    /// Enable hot reload in development
    pub hot_reload: bool,

    /// Template file extensions to watch
    pub watch_extensions: Vec<String>,
}

impl Default for TemplateSettings {
    fn default() -> Self {
        Self {
            template_dir: PathBuf::from("./templates"),
            cache_enabled: true,
            hot_reload: cfg!(debug_assertions),
            watch_extensions: vec!["html".to_string(), "jinja".to_string()],
        }
    }
}

/// Security configuration for HTMX applications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecuritySettings {
    /// Enable CSRF protection
    pub csrf_enabled: bool,

    /// Session maximum age in seconds
    pub session_max_age_secs: u64,

    /// Enable secure cookies (HTTPS only)
    pub secure_cookies: bool,

    /// Cookie `SameSite` policy
    pub same_site: SameSitePolicy,

    /// Enable security headers middleware
    pub security_headers_enabled: bool,

    /// Rate limiting configuration for HTMX routes
    pub rate_limit: HtmxRateLimitConfig,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            csrf_enabled: true,
            session_max_age_secs: 86400, // 24 hours
            secure_cookies: !cfg!(debug_assertions),
            same_site: SameSitePolicy::Lax,
            security_headers_enabled: true,
            rate_limit: HtmxRateLimitConfig::default(),
        }
    }
}

/// Cookie `SameSite` policy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SameSitePolicy {
    /// Strict `SameSite` policy
    Strict,
    /// Lax `SameSite` policy (recommended)
    Lax,
    /// None `SameSite` policy (requires secure cookies)
    None,
}

/// HTMX-specific rate limiting configuration
///
/// This extends acton-service's rate limiting with HTMX-specific options
/// like per-route limits for authentication endpoints.
///
/// # Example Configuration
///
/// ```toml
/// [security.rate_limit]
/// enabled = true
/// per_ip_rpm = 60              # 60 requests per minute per IP address
/// per_route_rpm = 30           # 30 requests per minute for specific routes (e.g., auth)
/// redis_enabled = true         # Use Redis for distributed rate limiting
/// failure_mode = "closed"      # Deny on rate limit errors (strict)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HtmxRateLimitConfig {
    /// Enable HTMX-specific rate limiting
    pub enabled: bool,

    /// Requests per minute per IP address (for anonymous requests)
    pub per_ip_rpm: u32,

    /// Requests per minute per authenticated user
    pub per_user_rpm: u32,

    /// Requests per minute for specific routes (e.g., auth endpoints)
    pub per_route_rpm: u32,

    /// Rate limit window in seconds
    pub window_secs: u64,

    /// Use Redis for distributed rate limiting (requires cache feature)
    /// Falls back to in-memory if Redis is unavailable
    pub redis_enabled: bool,

    /// Failure mode when rate limit backend fails
    /// - Closed: Deny requests when rate limiting fails (strict, production)
    /// - Open: Allow requests when rate limiting fails (permissive, development)
    pub failure_mode: RateLimitFailureMode,

    /// Route patterns that should use stricter rate limits (e.g., `"/login"`, `"/register"`)
    pub strict_routes: Vec<String>,
}

impl Default for HtmxRateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            per_ip_rpm: 60,
            per_user_rpm: 120,
            per_route_rpm: 30,
            window_secs: 60,
            redis_enabled: cfg!(feature = "redis"),
            failure_mode: RateLimitFailureMode::default(),
            strict_routes: vec![
                "/login".to_string(),
                "/register".to_string(),
                "/password-reset".to_string(),
            ],
        }
    }
}

/// Failure mode for rate limit backend errors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitFailureMode {
    /// Deny requests when rate limiting fails (strict, production)
    Closed,
    /// Allow requests when rate limiting fails (permissive, development)
    Open,
}

impl Default for RateLimitFailureMode {
    fn default() -> Self {
        if cfg!(debug_assertions) {
            Self::Open
        } else {
            Self::Closed
        }
    }
}

/// Failure mode for policy evaluation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailureMode {
    /// Deny requests when policy evaluation fails (strict)
    Closed,
    /// Allow requests when policy evaluation fails (permissive)
    Open,
}

impl Default for FailureMode {
    fn default() -> Self {
        Self::Closed
    }
}

/// Cedar authorization configuration for HTMX applications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CedarConfig {
    /// Enable Cedar authorization
    pub enabled: bool,
    /// Path to Cedar policy file
    pub policy_path: PathBuf,
    /// Enable policy hot-reload (watch file for changes)
    pub hot_reload: bool,
    /// Failure mode for policy evaluation errors
    pub failure_mode: FailureMode,
    /// Enable policy caching for better performance
    pub cache_enabled: bool,
}

impl Default for CedarConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            policy_path: PathBuf::from("./policies/main.cedar"),
            hot_reload: cfg!(debug_assertions),
            failure_mode: FailureMode::default(),
            cache_enabled: true,
        }
    }
}

/// HTMX-specific configuration extension for acton-service
///
/// This struct is used as the custom extension type `T` in `acton_service::Config<T>`.
/// All fields here are flattened into the root config.toml alongside framework fields.
///
/// # Example
///
/// ```rust,ignore
/// use acton_dx::htmx::config::ActonHtmxConfig;
/// use acton_service::prelude::Config;
///
/// // Load full config with HTMX extensions
/// let config: Config<ActonHtmxConfig> = Config::load_for_service("my-app")?;
///
/// // Access framework config
/// println!("Port: {}", config.service.port);
/// println!("DB: {:?}", config.database_url());
///
/// // Access HTMX extensions via .custom
/// println!("CSRF: {}", config.custom.security.csrf_enabled);
/// println!("Templates: {:?}", config.custom.templates.template_dir);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActonHtmxConfig {
    /// HTMX-specific settings
    #[serde(default)]
    pub htmx: HtmxSettings,

    /// Template engine settings
    #[serde(default)]
    pub templates: TemplateSettings,

    /// Security settings (HTMX-specific, complements acton-service security)
    #[serde(default)]
    pub security: SecuritySettings,

    /// OAuth2 configuration
    #[serde(default)]
    pub oauth2: OAuthConfig,

    /// Services transport configuration
    ///
    /// Configures how the application communicates with microservices.
    /// IPC (Unix Domain Sockets) is the default for co-located services.
    /// gRPC is available for distributed deployments.
    ///
    /// # Example
    ///
    /// ```toml
    /// [services]
    /// transport_type = "ipc"  # or "grpc"
    ///
    /// [services.ipc]
    /// app_name = "my-app"
    /// timeout_ms = 30000
    ///
    /// [services.grpc]
    /// auth_endpoint = "http://localhost:50051"
    /// ```
    #[cfg(feature = "microservices")]
    #[serde(default)]
    pub services: crate::htmx::clients::TransportConfig,

    /// Feature flags
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[cfg(feature = "microservices")]
impl ActonHtmxConfig {
    /// Check if IPC transport is configured (default).
    #[must_use]
    pub const fn is_ipc_transport(&self) -> bool {
        matches!(
            self.services.transport_type,
            crate::htmx::clients::TransportType::Ipc
        )
    }

    /// Check if gRPC transport is configured.
    #[must_use]
    pub const fn is_grpc_transport(&self) -> bool {
        matches!(
            self.services.transport_type,
            crate::htmx::clients::TransportType::Grpc
        )
    }

    /// Get the transport type.
    #[must_use]
    pub const fn transport_type(&self) -> crate::htmx::clients::TransportType {
        self.services.transport_type
    }
}

// Re-export the old RateLimitConfig name for backwards compatibility during transition
// TODO: Remove after updating all usages
pub use HtmxRateLimitConfig as RateLimitConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ActonHtmxConfig::default();
        assert_eq!(config.htmx.request_timeout_ms, 5000);
        assert!(config.htmx.history_enabled);
        assert!(config.htmx.auto_vary);
        assert!(config.security.csrf_enabled);
        assert_eq!(config.security.session_max_age_secs, 86400);
    }

    #[test]
    fn test_template_defaults() {
        let templates = TemplateSettings::default();
        assert_eq!(templates.template_dir, PathBuf::from("./templates"));
        assert!(templates.cache_enabled);
        assert_eq!(templates.watch_extensions, vec!["html", "jinja"]);
    }

    #[test]
    fn test_security_defaults() {
        let security = SecuritySettings::default();
        assert!(security.csrf_enabled);
        assert!(security.security_headers_enabled);

        // secure_cookies should be true in release, false in debug
        #[cfg(debug_assertions)]
        assert!(!security.secure_cookies);

        #[cfg(not(debug_assertions))]
        assert!(security.secure_cookies);
    }

    #[test]
    fn test_rate_limit_defaults() {
        let rate_limit = HtmxRateLimitConfig::default();
        assert!(rate_limit.enabled);
        assert_eq!(rate_limit.per_ip_rpm, 60);
        assert_eq!(rate_limit.per_route_rpm, 30);
        assert!(!rate_limit.strict_routes.is_empty());
    }
}

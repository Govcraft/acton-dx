//! Configuration management for acton-dx
//!
//! Extends acton-service's XDG-compliant configuration system with HTMX-specific
//! settings. Configuration is loaded from multiple sources with clear precedence:
//!
//! 1. Environment variables (highest priority, `ACTON_` prefix, `__` for nesting)
//! 2. `./config.toml` (development)
//! 3. `~/.config/acton-dx/config.toml` (user config, XDG)
//! 4. `/etc/acton-dx/config.toml` (system config)
//! 5. Hardcoded defaults (fallback)
//!
//! Environment variable format: `ACTON_SECTION__FIELD_NAME`
//! - Use `__` (double underscore) to separate nested sections
//! - Use `_` (single underscore) within field names
//! - Example: `ACTON_HTMX__REQUEST_TIMEOUT_MS=5000`
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
//! url = "sqlite://./dev.db"
//! optional = true
//! lazy_init = true
//!
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
//! ```rust
//! use acton_htmx::config::ActonHtmxConfig;
//!
//! // Load default configuration
//! let config = ActonHtmxConfig::default();
//!
//! // Access HTMX-specific config
//! let timeout = config.htmx.request_timeout_ms;
//! let csrf_enabled = config.security.csrf_enabled;
//! ```

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(feature = "cedar")]
use std::time::Duration;

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

/// Security configuration
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

    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            csrf_enabled: true,
            session_max_age_secs: 86400, // 24 hours
            secure_cookies: !cfg!(debug_assertions),
            same_site: SameSitePolicy::Lax,
            security_headers_enabled: true,
            rate_limit: RateLimitConfig::default(),
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

/// Rate limiting configuration
///
/// Supports both Redis-backed (distributed) and in-memory (single instance) rate limiting.
/// Rate limits can be configured per authenticated user, per IP address, and per specific route patterns.
///
/// # Example Configuration
///
/// ```toml
/// [security.rate_limit]
/// enabled = true
/// per_user_rpm = 120           # 120 requests per minute for authenticated users
/// per_ip_rpm = 60              # 60 requests per minute per IP address
/// per_route_rpm = 30           # 30 requests per minute for specific routes (e.g., auth)
/// window_secs = 60             # Rate limit window (60 seconds)
/// redis_enabled = true         # Use Redis for distributed rate limiting
/// failure_mode = "closed"      # Deny on rate limit errors (strict)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    /// Enable rate limiting middleware
    pub enabled: bool,

    /// Requests per minute per authenticated user
    pub per_user_rpm: u32,

    /// Requests per minute per IP address (for anonymous requests)
    pub per_ip_rpm: u32,

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

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            per_user_rpm: 120,
            per_ip_rpm: 60,
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

/// Failure mode for Cedar policy evaluation errors
#[cfg(feature = "cedar")]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FailureMode {
    /// Deny requests when policy evaluation fails (strict, production)
    Closed,
    /// Allow requests when policy evaluation fails (permissive, development)
    Open,
}

#[cfg(feature = "cedar")]
impl Default for FailureMode {
    fn default() -> Self {
        if cfg!(debug_assertions) {
            Self::Open
        } else {
            Self::Closed
        }
    }
}

/// Cedar authorization configuration
///
/// Configuration for AWS Cedar policy-based authorization.
/// Cedar provides declarative, human-readable authorization policies
/// with support for RBAC, ABAC, resource ownership, and attribute-based access control.
///
/// # Example Configuration
///
/// ```toml
/// [cedar]
/// enabled = true
/// policy_path = "policies/app.cedar"
/// hot_reload = false
/// hot_reload_interval_secs = 60
/// cache_enabled = true
/// cache_ttl_secs = 300
/// failure_mode = "closed"  # or "open"
/// ```
#[cfg(feature = "cedar")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CedarConfig {
    /// Enable Cedar authorization
    pub enabled: bool,

    /// Path to Cedar policy file
    pub policy_path: PathBuf,

    /// Enable policy hot-reload (watch file for changes)
    /// Note: Manual reload via endpoint is recommended in production
    pub hot_reload: bool,

    /// Hot-reload check interval in seconds
    pub hot_reload_interval_secs: u64,

    /// Enable policy caching for performance (requires redis feature)
    pub cache_enabled: bool,

    /// Policy cache TTL in seconds
    pub cache_ttl_secs: u64,

    /// Failure mode for policy evaluation errors
    /// - Closed: Deny requests when policy evaluation fails (strict, production)
    /// - Open: Allow requests when policy evaluation fails (permissive, development)
    pub failure_mode: FailureMode,
}

#[cfg(feature = "cedar")]
impl Default for CedarConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default, must be explicitly enabled
            policy_path: PathBuf::from("policies/app.cedar"),
            hot_reload: false,
            hot_reload_interval_secs: 60,
            cache_enabled: true,
            cache_ttl_secs: 300,
            failure_mode: FailureMode::default(), // Open in debug, closed in release
        }
    }
}

#[cfg(feature = "cedar")]
impl CedarConfig {
    /// Get hot-reload interval as Duration
    #[must_use]
    pub const fn hot_reload_interval(&self) -> Duration {
        Duration::from_secs(self.hot_reload_interval_secs)
    }

    /// Get cache TTL as Duration
    #[must_use]
    pub const fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.cache_ttl_secs)
    }
}

/// Complete acton-dx configuration
///
/// Combines framework configuration with HTMX-specific settings.
/// Uses `#[serde(flatten)]` to merge all fields into a single `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActonHtmxConfig {
    /// HTMX-specific settings
    #[serde(default)]
    pub htmx: HtmxSettings,

    /// Template engine settings
    #[serde(default)]
    pub templates: TemplateSettings,

    /// Security settings
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

    /// Cedar authorization configuration (optional, requires cedar feature)
    #[cfg(feature = "cedar")]
    #[serde(default)]
    pub cedar: Option<CedarConfig>,

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

impl ActonHtmxConfig {
    /// Load configuration for a specific service
    ///
    /// Searches for configuration in XDG-compliant locations with precedence:
    /// 1. Environment variables (`ACTON_*`, use `__` for nesting)
    /// 2. `./config.toml`
    /// 3. `~/.config/acton-dx/{service_name}/config.toml`
    /// 4. `/etc/acton-dx/{service_name}/config.toml`
    /// 5. Defaults
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Default configuration cannot be serialized to TOML
    /// - Configuration file cannot be read or parsed
    /// - Configuration values fail validation or type conversion
    /// - Required fields are missing from merged configuration
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use acton_htmx::config::ActonHtmxConfig;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// let config = ActonHtmxConfig::load_for_service("my-app")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_for_service(service_name: &str) -> anyhow::Result<Self> {
        let mut figment = Figment::new()
            // 5. Start with defaults (lowest priority)
            .merge(Toml::string(&toml::to_string(&Self::default())?));

        // 4. System config: /etc/acton-dx/{service_name}/config.toml
        let system_config = PathBuf::from("/etc/acton-dx")
            .join(service_name)
            .join("config.toml");
        if system_config.exists() {
            figment = figment.merge(Toml::file(&system_config));
        }

        // 3. User config: ~/.config/acton-dx/{service_name}/config.toml
        let user_config = Self::recommended_path(service_name);
        if user_config.exists() {
            figment = figment.merge(Toml::file(&user_config));
        }

        // 2. Local config: ./config.toml
        let local_config = PathBuf::from("./config.toml");
        if local_config.exists() {
            figment = figment.merge(Toml::file(&local_config));
        }

        // 1. Environment variables (highest priority, double underscore for nesting)
        figment = figment.merge(Env::prefixed("ACTON_").split("__").lowercase(true));

        let config = figment.extract()?;
        Ok(config)
    }

    /// Load configuration from a specific file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Default configuration cannot be serialized to TOML
    /// - Configuration file at `path` cannot be read or does not exist
    /// - Configuration file contains invalid TOML syntax
    /// - Configuration values fail validation or type conversion
    /// - Required fields are missing
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use acton_htmx::config::ActonHtmxConfig;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// let config = ActonHtmxConfig::load_from("./config/production.toml")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_from(path: &str) -> anyhow::Result<Self> {
        let config = Figment::new()
            // Start with defaults
            .merge(Toml::string(&toml::to_string(&Self::default())?))
            // Load from specified file (if it exists)
            .merge(Toml::file(path))
            // Environment variables override everything (prefix ACTON_, double underscore for nesting)
            .merge(Env::prefixed("ACTON_").split("__").lowercase(true))
            .extract()?;

        Ok(config)
    }

    /// Get the recommended XDG config path for a service
    ///
    /// # Example
    ///
    /// ```rust
    /// use acton_htmx::config::ActonHtmxConfig;
    ///
    /// let path = ActonHtmxConfig::recommended_path("my-app");
    /// // Returns: ~/.config/acton-dx/my-app/config.toml
    /// ```
    #[must_use]
    pub fn recommended_path(service_name: &str) -> PathBuf {
        dirs::config_dir().map_or_else(
            || PathBuf::from("./config.toml"),
            |config_dir| {
                config_dir
                    .join("acton-dx")
                    .join(service_name)
                    .join("config.toml")
            },
        )
    }

    /// Create config directory for a service
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Directory creation fails due to insufficient permissions
    /// - Parent directory path is invalid or inaccessible
    /// - Filesystem I/O error occurs
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use acton_htmx::config::ActonHtmxConfig;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// ActonHtmxConfig::create_config_dir("my-app")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_config_dir(service_name: &str) -> anyhow::Result<PathBuf> {
        let config_path = Self::recommended_path(service_name);
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(config_path)
    }
}

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
    fn test_recommended_path() {
        let path = ActonHtmxConfig::recommended_path("test-app");

        // Should contain the service name
        assert!(path.to_str().unwrap().contains("test-app"));

        // Should end with config.toml
        assert!(path.to_str().unwrap().ends_with("config.toml"));

        // Should contain acton-dx in the path
        assert!(path.to_str().unwrap().contains("acton-dx"));
    }

    #[test]
    fn test_load_from_nonexistent_file() {
        use std::env;
        // Ensure no test env vars from other tests
        env::remove_var("ACTON_HTMX__REQUEST_TIMEOUT_MS");
        env::remove_var("ACTON_HTMX__HISTORY_ENABLED");

        // Should return default config when file doesn't exist
        let result = ActonHtmxConfig::load_from("/nonexistent/path/config.toml");
        assert!(result.is_ok());

        let config = result.unwrap();
        // Should have default values
        assert_eq!(config.htmx.request_timeout_ms, 5000);
    }

    #[test]
    fn test_load_from_toml_file() {
        use std::env;
        use std::fs;
        use std::io::Write;

        // Ensure no test env vars from other tests
        env::remove_var("ACTON_HTMX__REQUEST_TIMEOUT_MS");
        env::remove_var("ACTON_HTMX__HISTORY_ENABLED");

        // Create a temporary config file
        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join("test_config.toml");

        let toml_content = r"
[htmx]
request_timeout_ms = 10000
history_enabled = false

[security]
csrf_enabled = false
session_max_age_secs = 3600
";

        let mut file = fs::File::create(&config_path).unwrap();
        file.write_all(toml_content.as_bytes()).unwrap();

        // Load configuration
        let result = ActonHtmxConfig::load_from(config_path.to_str().unwrap());
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.htmx.request_timeout_ms, 10000);
        assert!(!config.htmx.history_enabled);
        assert!(!config.security.csrf_enabled);
        assert_eq!(config.security.session_max_age_secs, 3600);

        // Cleanup
        fs::remove_file(config_path).ok();
    }

    #[test]
    fn test_load_for_service_with_defaults() {
        use std::env;
        // Ensure no test env vars from other tests
        env::remove_var("ACTON_HTMX__REQUEST_TIMEOUT_MS");
        env::remove_var("ACTON_HTMX__HISTORY_ENABLED");

        // Loading a service with no config files should return defaults
        let result = ActonHtmxConfig::load_for_service("nonexistent-service-123");
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.htmx.request_timeout_ms, 5000);
        assert!(config.htmx.history_enabled);
    }

    #[test]
    fn test_create_config_dir() {
        use std::fs;

        let temp_service = format!("test-service-{}", std::process::id());

        // This should create the directory structure
        let result = ActonHtmxConfig::create_config_dir(&temp_service);
        assert!(result.is_ok());

        let config_path = result.unwrap();

        // Parent directory should exist
        if let Some(parent) = config_path.parent() {
            assert!(parent.exists() || !config_path.to_str().unwrap().starts_with('/'));
        }

        // Cleanup - try to remove, but don't fail if we can't
        if let Some(parent) = config_path.parent() {
            if parent.exists() {
                fs::remove_dir_all(parent).ok();
            }
        }
    }
}

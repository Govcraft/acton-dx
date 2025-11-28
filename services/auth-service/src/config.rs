//! Configuration for the auth service.

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;

/// Auth service configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthServiceConfig {
    /// Service configuration.
    pub service: ServiceConfig,
    /// Session configuration.
    pub session: SessionConfig,
    /// CSRF configuration.
    pub csrf: CsrfConfig,
    /// Password hashing configuration.
    pub password: PasswordConfig,
}

/// Service endpoint configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Host to bind to.
    #[serde(default = "default_host")]
    pub host: String,
}

/// Session configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionConfig {
    /// Default session TTL in seconds.
    #[serde(default = "default_session_ttl")]
    pub default_ttl_seconds: u64,
    /// Maximum session TTL in seconds.
    #[serde(default = "default_max_session_ttl")]
    pub max_ttl_seconds: u64,
    /// Cleanup interval in seconds.
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_seconds: u64,
}

/// CSRF configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct CsrfConfig {
    /// Token TTL in seconds.
    #[serde(default = "default_csrf_ttl")]
    pub token_ttl_seconds: u64,
    /// Token length in bytes.
    #[serde(default = "default_token_bytes")]
    pub token_bytes: usize,
}

/// Password hashing configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct PasswordConfig {
    /// Argon2 memory cost in KiB.
    #[serde(default = "default_memory_cost")]
    pub memory_cost: u32,
    /// Argon2 time cost (iterations).
    #[serde(default = "default_time_cost")]
    pub time_cost: u32,
    /// Argon2 parallelism factor.
    #[serde(default = "default_parallelism")]
    pub parallelism: u32,
    /// Output hash length in bytes.
    #[serde(default = "default_hash_length")]
    pub hash_length: usize,
}

// Default value functions
const fn default_port() -> u16 {
    9001
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_session_ttl() -> u64 {
    3600 // 1 hour
}

const fn default_max_session_ttl() -> u64 {
    86400 // 24 hours
}

const fn default_cleanup_interval() -> u64 {
    300 // 5 minutes
}

const fn default_csrf_ttl() -> u64 {
    3600 // 1 hour
}

const fn default_token_bytes() -> usize {
    32
}

const fn default_memory_cost() -> u32 {
    19456 // OWASP recommended minimum
}

const fn default_time_cost() -> u32 {
    2
}

const fn default_parallelism() -> u32 {
    1
}

const fn default_hash_length() -> usize {
    32
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            default_ttl_seconds: default_session_ttl(),
            max_ttl_seconds: default_max_session_ttl(),
            cleanup_interval_seconds: default_cleanup_interval(),
        }
    }
}

impl Default for CsrfConfig {
    fn default() -> Self {
        Self {
            token_ttl_seconds: default_csrf_ttl(),
            token_bytes: default_token_bytes(),
        }
    }
}

impl Default for PasswordConfig {
    fn default() -> Self {
        Self {
            memory_cost: default_memory_cost(),
            time_cost: default_time_cost(),
            parallelism: default_parallelism(),
            hash_length: default_hash_length(),
        }
    }
}

impl AuthServiceConfig {
    /// Load configuration from files and environment.
    ///
    /// # Errors
    ///
    /// Returns an error if configuration cannot be loaded.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file("config/local.toml"))
            .merge(Env::prefixed("AUTH_SERVICE_").split("__"))
            .extract()
            .map_err(Box::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AuthServiceConfig::default();
        assert_eq!(config.service.port, 9001);
        assert_eq!(config.session.default_ttl_seconds, 3600);
        assert_eq!(config.csrf.token_bytes, 32);
        assert_eq!(config.password.memory_cost, 19456);
    }
}

//! Configuration for the cache service.

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

/// Service configuration.
#[derive(Debug, Deserialize)]
pub struct CacheServiceConfig {
    /// Redis configuration.
    pub redis: RedisConfig,
    /// Service configuration.
    #[serde(default)]
    pub service: ServiceConfig,
}

/// Redis configuration.
#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL.
    #[serde(default = "default_redis_url")]
    pub url: String,
    /// Connection pool size.
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    /// Connection timeout in seconds.
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_seconds: u64,
}

/// Service network configuration.
#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    /// Host to bind to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_port() -> u16 {
    50054
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

const fn default_pool_size() -> u32 {
    10
}

const fn default_connect_timeout() -> u64 {
    5
}

impl CacheServiceConfig {
    /// Load configuration from files and environment.
    ///
    /// # Errors
    ///
    /// Returns error if configuration cannot be loaded or parsed.
    pub fn load() -> anyhow::Result<Self> {
        let figment = Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file("config/local.toml"))
            .merge(Env::prefixed("CACHE_SERVICE_").split("__"));

        let config: Self = figment.extract()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_service_config() {
        let config = ServiceConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 50054);
    }
}

//! Configuration for the data service.

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

/// Service configuration.
#[derive(Debug, Deserialize)]
pub struct DataServiceConfig {
    /// Database configuration.
    pub database: DatabaseConfig,
    /// Service configuration.
    #[serde(default)]
    pub service: ServiceConfig,
}

/// Database configuration.
#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL (sqlite://... or postgres://...).
    pub url: String,
    /// Maximum connections in the pool.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Minimum connections in the pool.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
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
    50052
}

const fn default_max_connections() -> u32 {
    10
}

const fn default_min_connections() -> u32 {
    1
}

const fn default_connect_timeout() -> u64 {
    30
}

impl DataServiceConfig {
    /// Load configuration from files and environment.
    ///
    /// # Errors
    ///
    /// Returns error if configuration cannot be loaded or parsed.
    pub fn load() -> anyhow::Result<Self> {
        let figment = Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file("config/local.toml"))
            .merge(Env::prefixed("DATA_SERVICE_").split("__"));

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
        assert_eq!(config.port, 50052);
    }
}

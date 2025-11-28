//! Configuration for the Cedar authorization service.

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

/// Service configuration.
#[derive(Debug, Deserialize)]
pub struct CedarServiceConfig {
    /// Policy configuration.
    pub policies: PolicyConfig,
    /// Service configuration.
    #[serde(default)]
    pub service: ServiceConfig,
}

/// Policy configuration.
#[derive(Debug, Deserialize)]
pub struct PolicyConfig {
    /// Path to the policies directory.
    #[serde(default = "default_policies_path")]
    pub path: String,
    /// Whether to watch for policy changes.
    #[serde(default)]
    pub watch: bool,
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
    50053
}

fn default_policies_path() -> String {
    "policies".to_string()
}

impl CedarServiceConfig {
    /// Load configuration from files and environment.
    ///
    /// # Errors
    ///
    /// Returns error if configuration cannot be loaded or parsed.
    pub fn load() -> anyhow::Result<Self> {
        let figment = Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file("config/local.toml"))
            .merge(Env::prefixed("CEDAR_SERVICE_").split("__"));

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
        assert_eq!(config.port, 50053);
    }
}

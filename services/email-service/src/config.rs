//! Configuration for the email service.

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

/// Service configuration.
#[derive(Debug, Deserialize)]
pub struct EmailServiceConfig {
    /// SMTP configuration.
    pub smtp: SmtpConfig,
    /// Service configuration.
    #[serde(default)]
    pub service: ServiceConfig,
}

/// SMTP configuration.
#[derive(Debug, Deserialize)]
pub struct SmtpConfig {
    /// SMTP server host.
    pub host: String,
    /// SMTP server port.
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    /// SMTP username (optional).
    pub username: Option<String>,
    /// SMTP password (optional).
    pub password: Option<String>,
    /// Use TLS.
    #[serde(default = "default_tls")]
    pub tls: bool,
    /// Default from address.
    pub from_address: Option<String>,
    /// Default from name.
    pub from_name: Option<String>,
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
    50055
}

const fn default_smtp_port() -> u16 {
    587
}

const fn default_tls() -> bool {
    true
}

impl EmailServiceConfig {
    /// Load configuration from files and environment.
    ///
    /// # Errors
    ///
    /// Returns error if configuration cannot be loaded or parsed.
    pub fn load() -> anyhow::Result<Self> {
        let figment = Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file("config/local.toml"))
            .merge(Env::prefixed("EMAIL_SERVICE_").split("__"));

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
        assert_eq!(config.port, 50055);
    }
}

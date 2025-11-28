//! Configuration for the file service.

use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

/// Service configuration.
#[derive(Debug, Deserialize)]
pub struct FileServiceConfig {
    /// Storage configuration.
    pub storage: StorageConfig,
    /// Service configuration.
    #[serde(default)]
    pub service: ServiceConfig,
    /// URL generation configuration.
    #[serde(default)]
    pub urls: UrlConfig,
}

/// Storage configuration.
#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type.
    #[serde(default = "default_backend")]
    pub backend: String,
    /// Base path for local storage.
    #[serde(default = "default_base_path")]
    pub base_path: String,
    /// Maximum file size in bytes.
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// Chunk size for streaming.
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
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

/// URL configuration.
#[derive(Debug, Deserialize)]
pub struct UrlConfig {
    /// Base URL for public files.
    #[serde(default = "default_public_url")]
    pub public_base_url: String,
    /// Secret key for signed URLs.
    pub signing_key: Option<String>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

impl Default for UrlConfig {
    fn default() -> Self {
        Self {
            public_base_url: default_public_url(),
            signing_key: None,
        }
    }
}

fn default_backend() -> String {
    "local".to_string()
}

fn default_base_path() -> String {
    "./data/files".to_string()
}

const fn default_max_file_size() -> u64 {
    100 * 1024 * 1024 // 100MB
}

const fn default_chunk_size() -> usize {
    64 * 1024 // 64KB
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_port() -> u16 {
    50056
}

fn default_public_url() -> String {
    "http://localhost:50056/files".to_string()
}

impl FileServiceConfig {
    /// Load configuration from files and environment.
    ///
    /// # Errors
    ///
    /// Returns error if configuration cannot be loaded or parsed.
    pub fn load() -> anyhow::Result<Self> {
        let figment = Figment::new()
            .merge(Toml::file("config/default.toml"))
            .merge(Toml::file("config/local.toml"))
            .merge(Env::prefixed("FILE_SERVICE_").split("__"));

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
        assert_eq!(config.port, 50056);
    }

    #[test]
    fn test_default_url_config() {
        let config = UrlConfig::default();
        assert!(config.public_base_url.contains("localhost"));
        assert!(config.signing_key.is_none());
    }
}

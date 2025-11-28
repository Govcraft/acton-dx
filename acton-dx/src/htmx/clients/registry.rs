//! Service registry for managing multiple service clients.

use super::{
    error::ClientError, AuthClient, CacheClient, CedarClient, DataClient, EmailClient, FileClient,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for service endpoints.
#[derive(Debug, Clone, Default)]
pub struct ServicesConfig {
    /// Auth service endpoint (e.g., "http://localhost:50051").
    pub auth_endpoint: Option<String>,
    /// Data service endpoint.
    pub data_endpoint: Option<String>,
    /// Cedar service endpoint.
    pub cedar_endpoint: Option<String>,
    /// Cache service endpoint.
    pub cache_endpoint: Option<String>,
    /// Email service endpoint.
    pub email_endpoint: Option<String>,
    /// File service endpoint.
    pub file_endpoint: Option<String>,
}

/// Registry for managing service client connections.
///
/// The registry lazily connects to services and provides access to clients.
/// Each client is wrapped in `Arc<RwLock<>>` for thread-safe access.
#[derive(Debug, Clone)]
pub struct ServiceRegistry {
    config: ServicesConfig,
    auth: Option<Arc<RwLock<AuthClient>>>,
    data: Option<Arc<RwLock<DataClient>>>,
    cedar: Option<Arc<RwLock<CedarClient>>>,
    cache: Option<Arc<RwLock<CacheClient>>>,
    email: Option<Arc<RwLock<EmailClient>>>,
    file: Option<Arc<RwLock<FileClient>>>,
}

impl ServiceRegistry {
    /// Create a new service registry from configuration.
    ///
    /// This will attempt to connect to all configured services.
    ///
    /// # Errors
    ///
    /// Returns error if any configured service fails to connect.
    pub async fn from_config(config: &ServicesConfig) -> Result<Self, ClientError> {
        let auth = if let Some(ref endpoint) = config.auth_endpoint {
            Some(Arc::new(RwLock::new(AuthClient::connect(endpoint).await?)))
        } else {
            None
        };

        let data = if let Some(ref endpoint) = config.data_endpoint {
            Some(Arc::new(RwLock::new(DataClient::connect(endpoint).await?)))
        } else {
            None
        };

        let cedar = if let Some(ref endpoint) = config.cedar_endpoint {
            Some(Arc::new(RwLock::new(CedarClient::connect(endpoint).await?)))
        } else {
            None
        };

        let cache = if let Some(ref endpoint) = config.cache_endpoint {
            Some(Arc::new(RwLock::new(CacheClient::connect(endpoint).await?)))
        } else {
            None
        };

        let email = if let Some(ref endpoint) = config.email_endpoint {
            Some(Arc::new(RwLock::new(EmailClient::connect(endpoint).await?)))
        } else {
            None
        };

        let file = if let Some(ref endpoint) = config.file_endpoint {
            Some(Arc::new(RwLock::new(FileClient::connect(endpoint).await?)))
        } else {
            None
        };

        Ok(Self {
            config: config.clone(),
            auth,
            data,
            cedar,
            cache,
            email,
            file,
        })
    }

    /// Get the auth client.
    ///
    /// # Errors
    ///
    /// Returns error if the auth service is not configured.
    pub fn auth(&self) -> Result<Arc<RwLock<AuthClient>>, ClientError> {
        self.auth
            .clone()
            .ok_or(ClientError::NotConfigured("auth"))
    }

    /// Get the data client.
    ///
    /// # Errors
    ///
    /// Returns error if the data service is not configured.
    pub fn data(&self) -> Result<Arc<RwLock<DataClient>>, ClientError> {
        self.data
            .clone()
            .ok_or(ClientError::NotConfigured("data"))
    }

    /// Get the cedar client.
    ///
    /// # Errors
    ///
    /// Returns error if the cedar service is not configured.
    pub fn cedar(&self) -> Result<Arc<RwLock<CedarClient>>, ClientError> {
        self.cedar
            .clone()
            .ok_or(ClientError::NotConfigured("cedar"))
    }

    /// Get the cache client.
    ///
    /// # Errors
    ///
    /// Returns error if the cache service is not configured.
    pub fn cache(&self) -> Result<Arc<RwLock<CacheClient>>, ClientError> {
        self.cache
            .clone()
            .ok_or(ClientError::NotConfigured("cache"))
    }

    /// Get the email client.
    ///
    /// # Errors
    ///
    /// Returns error if the email service is not configured.
    pub fn email(&self) -> Result<Arc<RwLock<EmailClient>>, ClientError> {
        self.email
            .clone()
            .ok_or(ClientError::NotConfigured("email"))
    }

    /// Get the file client.
    ///
    /// # Errors
    ///
    /// Returns error if the file service is not configured.
    pub fn file(&self) -> Result<Arc<RwLock<FileClient>>, ClientError> {
        self.file
            .clone()
            .ok_or(ClientError::NotConfigured("file"))
    }

    /// Check if the auth service is configured.
    #[must_use]
    pub const fn has_auth(&self) -> bool {
        self.config.auth_endpoint.is_some()
    }

    /// Check if the data service is configured.
    #[must_use]
    pub const fn has_data(&self) -> bool {
        self.config.data_endpoint.is_some()
    }

    /// Check if the cedar service is configured.
    #[must_use]
    pub const fn has_cedar(&self) -> bool {
        self.config.cedar_endpoint.is_some()
    }

    /// Check if the cache service is configured.
    #[must_use]
    pub const fn has_cache(&self) -> bool {
        self.config.cache_endpoint.is_some()
    }

    /// Check if the email service is configured.
    #[must_use]
    pub const fn has_email(&self) -> bool {
        self.config.email_endpoint.is_some()
    }

    /// Check if the file service is configured.
    #[must_use]
    pub const fn has_file(&self) -> bool {
        self.config.file_endpoint.is_some()
    }
}

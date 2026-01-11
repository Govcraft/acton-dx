//! Microservices file storage backend
//!
//! Stores files via the file-service gRPC endpoint instead of local filesystem.

#[cfg(feature = "microservices")]
use super::traits::FileStorage;
#[cfg(feature = "microservices")]
use super::types::{StorageError, StorageResult, StoredFile, UploadedFile};
#[cfg(feature = "microservices")]
use crate::htmx::clients::{FileClient, ServiceRegistry};
#[cfg(feature = "microservices")]
use async_trait::async_trait;
#[cfg(feature = "microservices")]
use std::collections::HashMap;
#[cfg(feature = "microservices")]
use std::sync::Arc;
#[cfg(feature = "microservices")]
use tokio::sync::RwLock;

/// File storage backend that uses the file microservice
///
/// Uses the file-service gRPC endpoint for file storage operations. This allows
/// centralized file storage with features like deduplication, CDN integration,
/// and advanced access control handled by the service.
///
/// # Example
///
/// ```rust,no_run
/// use acton_htmx::storage::{FileStorage, MicroservicesFileStorage, UploadedFile};
/// use acton_htmx::clients::{ServiceRegistry, ServicesConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ServicesConfig {
///     file_endpoint: Some("http://localhost:50057".to_string()),
///     ..Default::default()
/// };
/// let registry = ServiceRegistry::from_config(&config).await?;
/// let storage = MicroservicesFileStorage::new(&registry)?;
///
/// let file = UploadedFile::new("document.pdf", "application/pdf", vec![/* ... */]);
/// let stored = storage.store(file).await?;
/// println!("Stored with ID: {}", stored.id);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "microservices")]
#[derive(Clone)]
pub struct MicroservicesFileStorage {
    client: Arc<RwLock<FileClient>>,
}

#[cfg(feature = "microservices")]
impl std::fmt::Debug for MicroservicesFileStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicroservicesFileStorage")
            .field("client", &"FileClient")
            .finish()
    }
}

#[cfg(feature = "microservices")]
impl MicroservicesFileStorage {
    /// Create a new microservices file storage backend from a service registry
    ///
    /// # Errors
    ///
    /// Returns error if the file service is not configured in the registry.
    pub fn new(registry: &ServiceRegistry) -> Result<Self, crate::htmx::clients::ClientError> {
        let client = registry.file()?;
        Ok(Self { client })
    }

    /// Create from an existing file client
    #[must_use]
    pub const fn from_client(client: Arc<RwLock<FileClient>>) -> Self {
        Self { client }
    }
}

#[cfg(feature = "microservices")]
#[async_trait]
impl FileStorage for MicroservicesFileStorage {
    async fn store(&self, file: UploadedFile) -> StorageResult<StoredFile> {
        let result = {
            let mut client = self.client.write();
            client
                .upload(&file.filename, &file.content_type, file.data, HashMap::new())
                .await
                .map_err(|e| StorageError::Other(format!("File service error: {e}")))?
        };

        if result.success {
            let info = result.file.ok_or_else(|| {
                StorageError::Other("Upload succeeded but no file metadata returned".to_string())
            })?;

            Ok(StoredFile {
                id: info.id.clone(),
                filename: info.filename,
                content_type: info.content_type,
                size: u64::try_from(info.size).unwrap_or(0),
                storage_path: info.id, // Use ID as storage path for service-backed storage
            })
        } else {
            Err(StorageError::Other(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }

    async fn retrieve(&self, id: &str) -> StorageResult<Vec<u8>> {
        let result = {
            let mut client = self.client.write();
            client
                .download(id)
                .await
                .map_err(|e| StorageError::Other(format!("File service error: {e}")))?
        };

        Ok(result.data)
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        let success = {
            let mut client = self.client.write();
            client
                .delete(id)
                .await
                .map_err(|e| StorageError::Other(format!("File service error: {e}")))?
        };

        // Treat deletion as idempotent - don't error if file doesn't exist
        // whether success is true or false
        let _ = success;
        Ok(())
    }

    async fn url(&self, id: &str) -> StorageResult<String> {
        let url = {
            let mut client = self.client.write();
            client
                .get_public_url(id)
                .await
                .map_err(|e| StorageError::Other(format!("File service error: {e}")))?
        };

        Ok(url)
    }

    async fn exists(&self, id: &str) -> StorageResult<bool> {
        // Try to get metadata - if it succeeds, file exists
        let result = {
            let mut client = self.client.write();
            client.get_metadata(id).await
        };

        match result {
            Ok(_) => Ok(true),
            Err(crate::htmx::clients::ClientError::NotConfigured(_)) => {
                Err(StorageError::Other("File service not configured".to_string()))
            }
            Err(_) => Ok(false), // File not found or other error
        }
    }

    async fn get_metadata(&self, id: &str) -> StorageResult<StoredFile> {
        let info = {
            let mut client = self.client.write();
            client
                .get_metadata(id)
                .await
                .map_err(|e| StorageError::NotFound(format!("File not found: {e}")))?
        };

        Ok(StoredFile {
            id: info.id.clone(),
            filename: info.filename,
            content_type: info.content_type,
            size: u64::try_from(info.size).unwrap_or(0),
            storage_path: info.id,
        })
    }
}

#[cfg(all(test, feature = "microservices"))]
mod tests {
    // Note: Integration tests would require a running file service
    // Compile-time verification that types are correctly defined is implicit
}

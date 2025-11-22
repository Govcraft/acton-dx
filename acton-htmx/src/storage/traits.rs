//! File storage trait definitions

use super::types::{StorageResult, StoredFile, UploadedFile};
use async_trait::async_trait;

/// Abstraction for file storage backends
///
/// This trait provides a unified interface for storing, retrieving, and managing files
/// across different storage backends (local filesystem, S3, Azure Blob, etc.).
///
/// # Design Principles
///
/// - **Backend Agnostic**: Handlers don't need to know about storage implementation
/// - **Async First**: All operations are async for optimal I/O performance
/// - **Type Safe**: Strong types prevent common errors
/// - **Production Ready**: Built-in support for streaming, validation, and error handling
///
/// # Implementation Requirements
///
/// Implementations must:
/// - Generate unique identifiers for stored files (UUIDs recommended)
/// - Handle concurrent access safely
/// - Provide atomic operations where possible
/// - Clean up resources on errors
///
/// # Examples
///
/// ```rust,no_run
/// use acton_htmx::storage::{FileStorage, LocalFileStorage, UploadedFile};
/// use std::path::PathBuf;
///
/// # async fn example() -> anyhow::Result<()> {
/// // Create storage backend
/// let storage = LocalFileStorage::new(PathBuf::from("/var/uploads"))?;
///
/// // Store a file
/// let file = UploadedFile::new("avatar.png", "image/png", vec![/* ... */]);
/// let stored = storage.store(file).await?;
///
/// // Retrieve the file
/// let data = storage.retrieve(&stored.id).await?;
///
/// // Get file URL (for serving to clients)
/// let url = storage.url(&stored.id).await?;
///
/// // Delete when no longer needed
/// storage.delete(&stored.id).await?;
/// # Ok(())
/// # }
/// ```
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait FileStorage: Send + Sync {
    /// Stores an uploaded file and returns metadata about the stored file
    ///
    /// This method should:
    /// - Generate a unique ID for the file
    /// - Persist the file data to the storage backend
    /// - Return metadata that can be used to retrieve the file later
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend is unavailable
    /// - There's insufficient storage space
    /// - File I/O fails
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use acton_htmx::storage::{FileStorage, LocalFileStorage, UploadedFile};
    /// # use std::path::PathBuf;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let storage = LocalFileStorage::new(PathBuf::from("/tmp"))?;
    /// let file = UploadedFile::new("report.pdf", "application/pdf", vec![/* ... */]);
    /// let stored = storage.store(file).await?;
    /// println!("Stored with ID: {}", stored.id);
    /// # Ok(())
    /// # }
    /// ```
    async fn store(&self, file: UploadedFile) -> StorageResult<StoredFile>;

    /// Retrieves file data by ID
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file doesn't exist (`StorageError::NotFound`)
    /// - The storage backend is unavailable
    /// - File I/O fails
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use acton_htmx::storage::{FileStorage, LocalFileStorage};
    /// # use std::path::PathBuf;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let storage = LocalFileStorage::new(PathBuf::from("/tmp"))?;
    /// let data = storage.retrieve("550e8400-e29b-41d4-a716-446655440000").await?;
    /// println!("Retrieved {} bytes", data.len());
    /// # Ok(())
    /// # }
    /// ```
    async fn retrieve(&self, id: &str) -> StorageResult<Vec<u8>>;

    /// Deletes a file by ID
    ///
    /// This operation should be idempotent - deleting a non-existent file
    /// should not return an error.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend is unavailable
    /// - File I/O fails (permissions, etc.)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use acton_htmx::storage::{FileStorage, LocalFileStorage};
    /// # use std::path::PathBuf;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let storage = LocalFileStorage::new(PathBuf::from("/tmp"))?;
    /// storage.delete("550e8400-e29b-41d4-a716-446655440000").await?;
    /// println!("File deleted successfully");
    /// # Ok(())
    /// # }
    /// ```
    async fn delete(&self, id: &str) -> StorageResult<()>;

    /// Returns a URL for accessing the file
    ///
    /// The returned URL format depends on the storage backend:
    /// - Local storage: relative path (e.g., "/uploads/abc123/file.jpg")
    /// - S3: presigned URL or public URL
    /// - CDN: CDN URL
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file doesn't exist
    /// - URL generation fails (e.g., S3 presigning error)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use acton_htmx::storage::{FileStorage, LocalFileStorage};
    /// # use std::path::PathBuf;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let storage = LocalFileStorage::new(PathBuf::from("/tmp"))?;
    /// let url = storage.url("550e8400-e29b-41d4-a716-446655440000").await?;
    /// println!("File available at: {}", url);
    /// # Ok(())
    /// # }
    /// ```
    async fn url(&self, id: &str) -> StorageResult<String>;

    /// Checks if a file exists
    ///
    /// This is useful for validating file references before attempting retrieval.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use acton_htmx::storage::{FileStorage, LocalFileStorage};
    /// # use std::path::PathBuf;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let storage = LocalFileStorage::new(PathBuf::from("/tmp"))?;
    /// if storage.exists("550e8400-e29b-41d4-a716-446655440000").await? {
    ///     println!("File exists!");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn exists(&self, id: &str) -> StorageResult<bool>;

    /// Retrieves file metadata by ID
    ///
    /// This method retrieves only the metadata (filename, content type, size, etc.)
    /// without reading the actual file data. This is useful for serving files with
    /// proper Content-Type headers and other metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file doesn't exist (`StorageError::NotFound`)
    /// - The storage backend is unavailable
    /// - Metadata cannot be read
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use acton_htmx::storage::{FileStorage, LocalFileStorage};
    /// # use std::path::PathBuf;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let storage = LocalFileStorage::new(PathBuf::from("/tmp"))?;
    /// let metadata = storage.get_metadata("550e8400-e29b-41d4-a716-446655440000").await?;
    /// println!("Content-Type: {}", metadata.content_type);
    /// println!("Size: {} bytes", metadata.size);
    /// # Ok(())
    /// # }
    /// ```
    async fn get_metadata(&self, id: &str) -> StorageResult<StoredFile>;
}

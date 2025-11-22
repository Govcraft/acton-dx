//! Local filesystem storage implementation

use super::traits::FileStorage;
use super::types::{StorageError, StorageResult, StoredFile, UploadedFile};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// Local filesystem storage backend
///
/// Stores files in a local directory with UUID-based organization.
/// Each file is stored in a subdirectory based on the first 2 characters
/// of its UUID to avoid hitting filesystem limits on files per directory.
///
/// # Directory Structure
///
/// ```text
/// /var/uploads/
/// ├── 55/
/// │   └── 550e8400-e29b-41d4-a716-446655440000/
/// │       └── document.pdf
/// ├── a3/
/// │   └── a3bb189e-8bf9-4a9a-b5c7-9f9c3b8e5d7a/
/// │       └── image.png
/// ```
///
/// # Examples
///
/// ```rust,no_run
/// use acton_htmx::storage::{LocalFileStorage, FileStorage, UploadedFile};
/// use std::path::PathBuf;
///
/// # async fn example() -> anyhow::Result<()> {
/// // Create storage (creates directory if it doesn't exist)
/// let storage = LocalFileStorage::new(PathBuf::from("/var/uploads"))?;
///
/// // Store a file
/// let file = UploadedFile::new("photo.jpg", "image/jpeg", vec![/* ... */]);
/// let stored = storage.store(file).await?;
///
/// // File is now at: /var/uploads/55/550e8400.../photo.jpg
/// println!("Stored at: {}", stored.storage_path);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LocalFileStorage {
    /// Base directory for file storage
    base_path: PathBuf,
}

impl LocalFileStorage {
    /// Creates a new local file storage instance
    ///
    /// This will verify that the base path exists and is writable.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The base path doesn't exist and cannot be created
    /// - The base path is not a directory
    /// - Insufficient permissions to write to the directory
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use acton_htmx::storage::LocalFileStorage;
    /// use std::path::PathBuf;
    ///
    /// let storage = LocalFileStorage::new(PathBuf::from("/var/uploads"))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(base_path: PathBuf) -> StorageResult<Self> {
        // Validate base path (synchronous check is OK for initialization)
        if base_path.exists() && !base_path.is_dir() {
            return Err(StorageError::InvalidPath(format!(
                "{} is not a directory",
                base_path.display()
            )));
        }

        Ok(Self { base_path })
    }

    /// Gets the filesystem path for a file ID
    ///
    /// Returns the directory path where the file should be stored,
    /// using the first 2 characters of the ID as a prefix.
    fn get_file_directory(&self, id: &str) -> PathBuf {
        // Use first 2 chars of ID as subdirectory to avoid too many files in one dir
        let prefix = &id[..2.min(id.len())];
        self.base_path.join(prefix).join(id)
    }

    /// Gets the full filesystem path for a stored file
    fn get_file_path(&self, id: &str, filename: &str) -> PathBuf {
        self.get_file_directory(id).join(filename)
    }

    /// Ensures the storage directory exists
    async fn ensure_directory(&self, path: &Path) -> StorageResult<()> {
        fs::create_dir_all(path).await?;
        Ok(())
    }
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    async fn store(&self, file: UploadedFile) -> StorageResult<StoredFile> {
        // Generate unique ID
        let id = Uuid::new_v4().to_string();

        // Create directory structure
        let dir = self.get_file_directory(&id);
        self.ensure_directory(&dir).await?;

        // Write file to disk
        let file_path = self.get_file_path(&id, &file.filename);
        let mut f = fs::File::create(&file_path).await?;
        f.write_all(&file.data).await?;
        f.flush().await?;

        // Return metadata
        Ok(StoredFile {
            id: id.clone(),
            filename: file.filename.clone(),
            content_type: file.content_type.clone(),
            size: file.size(),
            storage_path: file_path.to_string_lossy().to_string(),
        })
    }

    async fn retrieve(&self, id: &str) -> StorageResult<Vec<u8>> {
        // We need to find the file by ID, but we don't know the filename
        // List files in the ID directory and read the first one
        let dir = self.get_file_directory(id);

        if !dir.exists() {
            return Err(StorageError::NotFound(id.to_string()));
        }

        let mut entries = fs::read_dir(&dir).await?;

        // Read the first file in the directory (there should only be one)
        if let Some(entry) = entries.next_entry().await? {
            let file_path = entry.path();
            if file_path.is_file() {
                let data = fs::read(&file_path).await?;
                return Ok(data);
            }
        }

        Err(StorageError::NotFound(id.to_string()))
    }

    async fn delete(&self, id: &str) -> StorageResult<()> {
        let dir = self.get_file_directory(id);

        // Idempotent - don't error if directory doesn't exist
        if dir.exists() {
            fs::remove_dir_all(&dir).await?;
        }

        Ok(())
    }

    async fn url(&self, id: &str) -> StorageResult<String> {
        // For local storage, return a relative URL path
        // In production, this would be served by the web server
        let dir = self.get_file_directory(id);

        if !dir.exists() {
            return Err(StorageError::NotFound(id.to_string()));
        }

        let mut entries = fs::read_dir(&dir).await?;

        if let Some(entry) = entries.next_entry().await? {
            let file_path = entry.path();
            if file_path.is_file() {
                // Return relative URL path from base
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| StorageError::InvalidPath(format!("Invalid filename in {id}")))?;

                // Use first 2 chars as prefix
                let prefix = &id[..2.min(id.len())];
                return Ok(format!("/uploads/{prefix}/{id}/{filename}"));
            }
        }

        Err(StorageError::NotFound(id.to_string()))
    }

    async fn exists(&self, id: &str) -> StorageResult<bool> {
        let dir = self.get_file_directory(id);
        Ok(dir.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (LocalFileStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalFileStorage::new(temp_dir.path().to_path_buf()).unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let (storage, _temp) = create_test_storage();

        let file = UploadedFile::new("test.txt", "text/plain", b"Hello, World!".to_vec());

        // Store
        let stored = storage.store(file).await.unwrap();
        assert!(!stored.id.is_empty());
        assert_eq!(stored.filename, "test.txt");
        assert_eq!(stored.size, 13);

        // Retrieve
        let data = storage.retrieve(&stored.id).await.unwrap();
        assert_eq!(data, b"Hello, World!");
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _temp) = create_test_storage();

        let file = UploadedFile::new("test.txt", "text/plain", b"Test".to_vec());
        let stored = storage.store(file).await.unwrap();

        // Verify exists
        assert!(storage.exists(&stored.id).await.unwrap());

        // Delete
        storage.delete(&stored.id).await.unwrap();

        // Verify doesn't exist
        assert!(!storage.exists(&stored.id).await.unwrap());

        // Delete again (idempotent)
        storage.delete(&stored.id).await.unwrap();
    }

    #[tokio::test]
    async fn test_url_generation() {
        let (storage, _temp) = create_test_storage();

        let file = UploadedFile::new("photo.jpg", "image/jpeg", b"fake image".to_vec());
        let stored = storage.store(file).await.unwrap();

        let url = storage.url(&stored.id).await.unwrap();
        assert!(url.starts_with("/uploads/"));
        assert!(url.contains(&stored.id));
        assert!(url.ends_with("/photo.jpg"));
    }

    #[tokio::test]
    async fn test_retrieve_nonexistent() {
        let (storage, _temp) = create_test_storage();

        let result = storage.retrieve("nonexistent-id").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_exists() {
        let (storage, _temp) = create_test_storage();

        // Nonexistent
        assert!(!storage.exists("nonexistent-id").await.unwrap());

        // Create file
        let file = UploadedFile::new("test.txt", "text/plain", b"Test".to_vec());
        let stored = storage.store(file).await.unwrap();

        // Should exist
        assert!(storage.exists(&stored.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_directory_structure() {
        let (storage, temp) = create_test_storage();

        let file = UploadedFile::new("test.txt", "text/plain", b"Test".to_vec());
        let stored = storage.store(file).await.unwrap();

        // Verify directory structure: base/prefix/id/filename
        let prefix = &stored.id[..2];
        let expected_path = temp.path().join(prefix).join(&stored.id).join("test.txt");
        assert!(expected_path.exists());
    }

    #[tokio::test]
    async fn test_invalid_base_path() {
        // Try to create storage with a file instead of directory
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("not-a-directory");
        std::fs::write(&file_path, b"test").unwrap();

        let result = LocalFileStorage::new(file_path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
    }
}

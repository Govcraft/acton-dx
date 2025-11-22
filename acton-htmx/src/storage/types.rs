//! Core types for file storage

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur during file storage operations
#[derive(Debug, Error)]
pub enum StorageError {
    /// File not found in storage
    #[error("File not found: {0}")]
    NotFound(String),

    /// I/O error during storage operation
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid file path or identifier
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Storage quota exceeded
    #[error("Storage quota exceeded")]
    QuotaExceeded,

    /// File size exceeds limit
    #[error("File size {actual} exceeds limit of {limit} bytes")]
    FileSizeExceeded {
        /// Actual file size
        actual: u64,
        /// Maximum allowed size
        limit: u64,
    },

    /// Invalid MIME type
    #[error("Invalid MIME type: expected {expected:?}, got {actual}")]
    InvalidMimeType {
        /// Expected MIME types
        expected: Vec<String>,
        /// Actual MIME type
        actual: String,
    },

    /// Generic storage error
    #[error("Storage error: {0}")]
    Other(String),
}

/// Result type for storage operations
pub type StorageResult<T> = Result<T, StorageError>;

/// A file that has been uploaded but not yet stored
///
/// This represents the in-memory state of an uploaded file before it's
/// persisted to the storage backend.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::storage::UploadedFile;
///
/// let file = UploadedFile {
///     filename: "document.pdf".to_string(),
///     content_type: "application/pdf".to_string(),
///     data: vec![0x25, 0x50, 0x44, 0x46], // PDF magic bytes
/// };
///
/// assert_eq!(file.size(), 4);
/// ```
#[derive(Debug, Clone)]
pub struct UploadedFile {
    /// Original filename from the upload
    pub filename: String,

    /// MIME content type (e.g., "image/png", "application/pdf")
    pub content_type: String,

    /// File data as bytes
    pub data: Vec<u8>,
}

impl UploadedFile {
    /// Creates a new uploaded file
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::UploadedFile;
    ///
    /// let file = UploadedFile::new(
    ///     "photo.jpg",
    ///     "image/jpeg",
    ///     vec![0xFF, 0xD8, 0xFF], // JPEG magic bytes
    /// );
    /// ```
    #[must_use]
    pub fn new(filename: impl Into<String>, content_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            filename: filename.into(),
            content_type: content_type.into(),
            data,
        }
    }

    /// Returns the size of the file in bytes
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::UploadedFile;
    ///
    /// let file = UploadedFile::new("test.txt", "text/plain", vec![1, 2, 3, 4, 5]);
    /// assert_eq!(file.size(), 5);
    /// ```
    #[must_use]
    pub fn size(&self) -> u64 {
        self.data.len() as u64
    }

    /// Validates the file size against a maximum limit
    ///
    /// # Errors
    ///
    /// Returns `StorageError::FileSizeExceeded` if the file is larger than `max_bytes`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::UploadedFile;
    ///
    /// let file = UploadedFile::new("test.txt", "text/plain", vec![1, 2, 3]);
    ///
    /// // Passes - file is 3 bytes, limit is 10
    /// assert!(file.validate_size(10).is_ok());
    ///
    /// // Fails - file is 3 bytes, limit is 2
    /// assert!(file.validate_size(2).is_err());
    /// ```
    pub fn validate_size(&self, max_bytes: u64) -> StorageResult<()> {
        let size = self.size();
        if size > max_bytes {
            return Err(StorageError::FileSizeExceeded {
                actual: size,
                limit: max_bytes,
            });
        }
        Ok(())
    }

    /// Validates the file's MIME type against an allowlist
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidMimeType` if the content type is not in `allowed_types`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::UploadedFile;
    ///
    /// let file = UploadedFile::new("photo.jpg", "image/jpeg", vec![]);
    ///
    /// // Passes - JPEG is in the allowlist
    /// assert!(file.validate_mime(&["image/jpeg", "image/png"]).is_ok());
    ///
    /// // Fails - JPEG is not in the allowlist of PNG-only
    /// assert!(file.validate_mime(&["image/png"]).is_err());
    /// ```
    pub fn validate_mime(&self, allowed_types: &[&str]) -> StorageResult<()> {
        if !allowed_types.contains(&self.content_type.as_str()) {
            return Err(StorageError::InvalidMimeType {
                expected: allowed_types.iter().map(|s| (*s).to_string()).collect(),
                actual: self.content_type.clone(),
            });
        }
        Ok(())
    }

    /// Extracts the file extension from the filename
    ///
    /// Returns `None` if the filename has no extension
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::UploadedFile;
    ///
    /// let file = UploadedFile::new("document.pdf", "application/pdf", vec![]);
    /// assert_eq!(file.extension(), Some("pdf"));
    ///
    /// let no_ext = UploadedFile::new("README", "text/plain", vec![]);
    /// assert_eq!(no_ext.extension(), None);
    /// ```
    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        let parts: Vec<&str> = self.filename.rsplitn(2, '.').collect();
        // If there's exactly 2 parts, we have an extension
        if parts.len() == 2 {
            Some(parts[0])
        } else {
            None
        }
    }
}

/// A file that has been stored in the backend
///
/// This represents metadata about a file that has been persisted to storage.
/// The actual file data is accessible via the storage backend.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::storage::StoredFile;
///
/// let stored = StoredFile {
///     id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
///     filename: "document.pdf".to_string(),
///     content_type: "application/pdf".to_string(),
///     size: 1024,
///     storage_path: "/uploads/550e8400/document.pdf".to_string(),
/// };
///
/// println!("File stored at: {}", stored.storage_path);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredFile {
    /// Unique identifier for the stored file (typically UUID)
    pub id: String,

    /// Original filename
    pub filename: String,

    /// MIME content type
    pub content_type: String,

    /// File size in bytes
    pub size: u64,

    /// Storage backend-specific path or key
    ///
    /// - For local storage: filesystem path
    /// - For S3: object key
    /// - For Azure: blob name
    pub storage_path: String,
}

impl StoredFile {
    /// Creates a new stored file metadata record
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::StoredFile;
    ///
    /// let stored = StoredFile::new(
    ///     "abc-123",
    ///     "photo.jpg",
    ///     "image/jpeg",
    ///     2048,
    ///     "/uploads/abc-123/photo.jpg",
    /// );
    /// ```
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        filename: impl Into<String>,
        content_type: impl Into<String>,
        size: u64,
        storage_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            filename: filename.into(),
            content_type: content_type.into(),
            size,
            storage_path: storage_path.into(),
        }
    }
}

impl fmt::Display for StoredFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StoredFile(id={}, filename={}, size={})",
            self.id, self.filename, self.size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uploaded_file_size() {
        let file = UploadedFile::new("test.txt", "text/plain", vec![1, 2, 3, 4, 5]);
        assert_eq!(file.size(), 5);
    }

    #[test]
    fn test_validate_size_pass() {
        let file = UploadedFile::new("test.txt", "text/plain", vec![1, 2, 3]);
        assert!(file.validate_size(10).is_ok());
        assert!(file.validate_size(3).is_ok());
    }

    #[test]
    fn test_validate_size_fail() {
        let file = UploadedFile::new("test.txt", "text/plain", vec![1, 2, 3, 4, 5]);
        let result = file.validate_size(3);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StorageError::FileSizeExceeded { actual: 5, limit: 3 }
        ));
    }

    #[test]
    fn test_validate_mime_pass() {
        let file = UploadedFile::new("photo.jpg", "image/jpeg", vec![]);
        assert!(file.validate_mime(&["image/jpeg", "image/png"]).is_ok());
    }

    #[test]
    fn test_validate_mime_fail() {
        let file = UploadedFile::new("photo.jpg", "image/jpeg", vec![]);
        let result = file.validate_mime(&["image/png", "image/gif"]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StorageError::InvalidMimeType { .. }
        ));
    }

    #[test]
    fn test_extension() {
        let file = UploadedFile::new("document.pdf", "application/pdf", vec![]);
        assert_eq!(file.extension(), Some("pdf"));

        let no_ext = UploadedFile::new("README", "text/plain", vec![]);
        assert_eq!(no_ext.extension(), None);

        let multiple_dots = UploadedFile::new("archive.tar.gz", "application/gzip", vec![]);
        assert_eq!(multiple_dots.extension(), Some("gz"));
    }

    #[test]
    fn test_stored_file_display() {
        let stored = StoredFile::new("abc-123", "test.pdf", "application/pdf", 1024, "/uploads/abc-123/test.pdf");
        let display = format!("{stored}");
        assert!(display.contains("abc-123"));
        assert!(display.contains("test.pdf"));
        assert!(display.contains("1024"));
    }
}

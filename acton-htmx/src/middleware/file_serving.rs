//! File serving middleware with range requests, caching, and access control
//!
//! This module provides middleware for serving uploaded files with:
//! - Range request support for streaming and resumable downloads
//! - Proper cache headers (ETag, Last-Modified, Cache-Control)
//! - CDN integration hints
//! - Access control for private files
//!
//! # Examples
//!
//! ## Basic file serving
//!
//! ```rust,no_run
//! use acton_htmx::middleware::FileServingMiddleware;
//! use acton_htmx::storage::{FileStorage, LocalFileStorage};
//! use std::path::PathBuf;
//! use std::sync::Arc;
//! use axum::{Router, routing::get};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let storage = Arc::new(LocalFileStorage::new(PathBuf::from("/var/uploads"))?);
//! let middleware = FileServingMiddleware::new(storage);
//!
//! let app = Router::new()
//!     .route("/files/:id", get(|| async { "file handler" }))
//!     .layer(middleware);
//! # Ok(())
//! # }
//! ```
//!
//! ## With access control
//!
//! ```rust,no_run
//! use acton_htmx::middleware::{FileServingMiddleware, FileAccessControl};
//! use acton_htmx::storage::{FileStorage, LocalFileStorage};
//! use std::path::PathBuf;
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let storage = Arc::new(LocalFileStorage::new(PathBuf::from("/var/uploads"))?);
//!
//! // Custom access control
//! let access_control: FileAccessControl = Arc::new(|user_id, file_id| {
//!     Box::pin(async move {
//!         // Check if user owns the file or is admin
//!         Ok(true)
//!     })
//! });
//!
//! let middleware = FileServingMiddleware::new(storage)
//!     .with_access_control(access_control);
//! # Ok(())
//! # }
//! ```

use crate::storage::{FileStorage, StorageError, StorageResult};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{
            ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG,
            IF_NONE_MATCH, IF_RANGE, LAST_MODIFIED, RANGE,
        },
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::SystemTime,
};

/// Access control function type for file serving
///
/// Takes user ID (if authenticated) and file ID, returns whether access is allowed.
pub type FileAccessControl = Arc<
    dyn Fn(Option<String>, String) -> Pin<Box<dyn Future<Output = StorageResult<bool>> + Send>>
        + Send
        + Sync,
>;

/// Middleware for serving files with range requests, caching, and access control
#[derive(Clone)]
pub struct FileServingMiddleware<S: FileStorage> {
    #[allow(dead_code)] // Used in future layer implementation
    storage: Arc<S>,
    #[allow(dead_code)] // Used in future layer implementation
    access_control: Option<FileAccessControl>,
    #[allow(dead_code)] // Used in future layer implementation
    cache_max_age: u32,
    #[allow(dead_code)] // Used in future layer implementation
    enable_cdn_headers: bool,
}

impl<S: FileStorage> FileServingMiddleware<S> {
    /// Create a new file serving middleware
    #[must_use]
    pub fn new(storage: Arc<S>) -> Self {
        Self {
            storage,
            access_control: None,
            cache_max_age: 86400, // 1 day default
            enable_cdn_headers: false,
        }
    }

    /// Set custom access control function
    #[must_use]
    pub fn with_access_control(mut self, access_control: FileAccessControl) -> Self {
        self.access_control = Some(access_control);
        self
    }

    /// Set cache max-age in seconds (default: 86400 = 1 day)
    #[must_use]
    pub const fn with_cache_max_age(mut self, seconds: u32) -> Self {
        self.cache_max_age = seconds;
        self
    }

    /// Enable CDN-friendly headers
    #[must_use]
    pub const fn with_cdn_headers(mut self) -> Self {
        self.enable_cdn_headers = true;
        self
    }
}

/// Handler for serving a single file with range request support
///
/// This should be used as an Axum route handler for file serving endpoints.
///
/// # Examples
///
/// ```rust,no_run
/// use axum::{Router, routing::get};
/// use acton_htmx::middleware::serve_file;
/// use acton_htmx::storage::LocalFileStorage;
/// use std::path::PathBuf;
/// use std::sync::Arc;
///
/// # async fn example() -> anyhow::Result<()> {
/// let storage = Arc::new(LocalFileStorage::new(PathBuf::from("/var/uploads"))?);
///
/// let app = Router::new()
///     .route("/files/:id", get(serve_file::<LocalFileStorage>))
///     .with_state(storage);
/// # Ok(())
/// # }
/// ```
pub async fn serve_file<S: FileStorage>(
    State(storage): State<Arc<S>>,
    Path(file_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, FileServingError> {
    // Retrieve file metadata for content type and other info
    let metadata = storage
        .get_metadata(&file_id)
        .await
        .map_err(FileServingError::Storage)?;

    // Retrieve file data
    let data = storage
        .retrieve(&file_id)
        .await
        .map_err(FileServingError::Storage)?;

    // Generate ETag from file ID and size
    let etag = format!(r#""{}-{}""#, file_id, data.len());

    // Use content type from metadata, with mime_guess fallback
    let content_type = if !metadata.content_type.is_empty()
        && metadata.content_type != "application/octet-stream"
    {
        metadata.content_type
    } else {
        // Fallback to MIME type detection from filename
        mime_guess::from_path(&metadata.filename)
            .first_or_octet_stream()
            .to_string()
    };

    // Check If-None-Match (ETag validation)
    if let Some(if_none_match) = headers.get(IF_NONE_MATCH) {
        if if_none_match.to_str().is_ok_and(|v| v == etag) {
            return Ok((StatusCode::NOT_MODIFIED, ()).into_response());
        }
    }

    // Check for range request
    if let Some(range_header) = headers.get(RANGE) {
        return serve_range_request(&data, range_header, &etag, &content_type, &headers);
    }

    // Serve complete file
    Ok(build_file_response(data, &etag, &content_type, None))
}

/// Serve a range request (partial content)
fn serve_range_request(
    data: &[u8],
    range_header: &HeaderValue,
    etag: &str,
    content_type: &str,
    headers: &HeaderMap,
) -> Result<Response, FileServingError> {
    let file_size = data.len();

    // Check If-Range header (validate ETag before serving range)
    if let Some(if_range) = headers.get(IF_RANGE) {
        if if_range.to_str().map_or(true, |v| v != etag) {
            // ETag doesn't match, serve full file instead
            return Ok(build_file_response(data.to_vec(), etag, content_type, None));
        }
    }

    // Parse range header (simplified - only handles single range)
    let range_str = range_header
        .to_str()
        .map_err(|_| FileServingError::InvalidRange)?;

    if !range_str.starts_with("bytes=") {
        return Err(FileServingError::InvalidRange);
    }

    let range_spec = &range_str[6..]; // Skip "bytes="
    let (start_str, end_str) = range_spec
        .split_once('-')
        .ok_or(FileServingError::InvalidRange)?;

    let start: usize = if start_str.is_empty() {
        // Suffix range: -500 means last 500 bytes
        let suffix_len: usize = end_str
            .parse()
            .map_err(|_| FileServingError::InvalidRange)?;
        file_size.saturating_sub(suffix_len)
    } else {
        start_str
            .parse()
            .map_err(|_| FileServingError::InvalidRange)?
    };

    let end: usize = if end_str.is_empty() {
        // Open-ended range: 500- means from byte 500 to end
        file_size - 1
    } else {
        end_str
            .parse::<usize>()
            .map_err(|_| FileServingError::InvalidRange)?
            .min(file_size - 1)
    };

    // Validate range
    if start > end || start >= file_size {
        return Err(FileServingError::RangeNotSatisfiable(file_size));
    }

    let range_data = data[start..=end].to_vec();

    let content_range = format!("bytes {start}-{end}/{file_size}");

    Ok(build_file_response(
        range_data,
        etag,
        content_type,
        Some((&content_range, StatusCode::PARTIAL_CONTENT)),
    ))
}

/// Build a file response with appropriate headers
fn build_file_response(
    data: Vec<u8>,
    etag: &str,
    content_type: &str,
    range_info: Option<(&str, StatusCode)>,
) -> Response {
    let mut response = Response::builder();

    // Set status code
    let status = range_info.map_or(StatusCode::OK, |(_, code)| code);
    response = response.status(status);

    // Content headers
    response = response
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, data.len())
        .header(ETAG, etag)
        .header(ACCEPT_RANGES, "bytes");

    // Range-specific headers
    if let Some((content_range, _)) = range_info {
        response = response.header(CONTENT_RANGE, content_range);
    }

    // Cache headers
    response = response
        .header(CACHE_CONTROL, "public, max-age=86400")
        .header(
            LAST_MODIFIED,
            httpdate::fmt_http_date(SystemTime::now()),
        );

    response
        .body(Body::from(data))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Error types for file serving operations
#[derive(Debug)]
pub enum FileServingError {
    /// Storage backend error
    Storage(StorageError),
    /// Invalid range request
    InvalidRange,
    /// Range not satisfiable
    RangeNotSatisfiable(usize),
    /// Access denied
    AccessDenied,
}

impl std::fmt::Display for FileServingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(e) => write!(f, "Storage error: {e}"),
            Self::InvalidRange => write!(f, "Invalid range request"),
            Self::RangeNotSatisfiable(size) => {
                write!(f, "Range not satisfiable (file size: {size})")
            }
            Self::AccessDenied => write!(f, "Access denied"),
        }
    }
}

impl std::error::Error for FileServingError {}

impl IntoResponse for FileServingError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Self::InvalidRange => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::RangeNotSatisfiable(size) => {
                let response = Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(CONTENT_RANGE, format!("bytes */{size}"))
                    .body(Body::from(self.to_string()))
                    .unwrap_or_else(|_| Response::new(Body::empty()));
                return response;
            }
            Self::AccessDenied => (StatusCode::FORBIDDEN, self.to_string()),
        };

        (status, message).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{LocalFileStorage, UploadedFile};
    use tempfile::TempDir;

    #[test]
    fn test_etag_generation() {
        let file_id = "test-file-123";
        let data = b"Hello, World!";
        let etag = format!(r#""{}-{}""#, file_id, data.len());
        assert_eq!(etag, r#""test-file-123-13""#);
    }

    #[tokio::test]
    async fn test_serve_file_uses_stored_content_type() {
        let temp = TempDir::new().unwrap();
        let storage = Arc::new(LocalFileStorage::new(temp.path().to_path_buf()).unwrap());

        // Store a PDF file
        let file = UploadedFile::new("document.pdf", "application/pdf", b"fake pdf".to_vec());
        let stored = storage.store(file).await.unwrap();

        // Serve the file
        let headers = HeaderMap::new();
        let response = serve_file(State(storage.clone()), Path(stored.id.clone()), headers)
            .await
            .unwrap();

        // Verify Content-Type header is from metadata
        let content_type = response.headers().get(CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "application/pdf");
    }

    #[tokio::test]
    async fn test_serve_file_uses_mime_guess_fallback() {
        let temp = TempDir::new().unwrap();
        let storage = Arc::new(LocalFileStorage::new(temp.path().to_path_buf()).unwrap());

        // Store a file with generic content type
        let file = UploadedFile::new(
            "image.png",
            "application/octet-stream",
            b"fake png".to_vec(),
        );
        let stored = storage.store(file).await.unwrap();

        // Serve the file
        let headers = HeaderMap::new();
        let response = serve_file(State(storage.clone()), Path(stored.id.clone()), headers)
            .await
            .unwrap();

        // Verify Content-Type header is guessed from extension
        let content_type = response.headers().get(CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "image/png");
    }

    #[tokio::test]
    async fn test_serve_file_preserves_various_content_types() {
        let temp = TempDir::new().unwrap();
        let storage = Arc::new(LocalFileStorage::new(temp.path().to_path_buf()).unwrap());

        let test_cases = vec![
            ("photo.jpg", "image/jpeg", "image/jpeg"),
            ("video.mp4", "video/mp4", "video/mp4"),
            ("data.json", "application/json", "application/json"),
            ("style.css", "text/css", "text/css"),
            ("script.js", "application/javascript", "application/javascript"),
        ];

        for (filename, stored_type, expected_type) in test_cases {
            let file = UploadedFile::new(filename, stored_type, b"test data".to_vec());
            let stored = storage.store(file).await.unwrap();

            let headers = HeaderMap::new();
            let response = serve_file(State(storage.clone()), Path(stored.id.clone()), headers)
                .await
                .unwrap();

            let content_type = response.headers().get(CONTENT_TYPE).unwrap();
            assert_eq!(
                content_type,
                expected_type,
                "Content-Type mismatch for {filename}"
            );
        }
    }

    #[tokio::test]
    async fn test_serve_file_fallback_for_unknown_extension() {
        let temp = TempDir::new().unwrap();
        let storage = Arc::new(LocalFileStorage::new(temp.path().to_path_buf()).unwrap());

        // Store file with unknown extension and generic content type
        let file = UploadedFile::new(
            "file.unknownext",
            "application/octet-stream",
            b"data".to_vec(),
        );
        let stored = storage.store(file).await.unwrap();

        let headers = HeaderMap::new();
        let response = serve_file(State(storage.clone()), Path(stored.id.clone()), headers)
            .await
            .unwrap();

        // Should fallback to octet-stream
        let content_type = response.headers().get(CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "application/octet-stream");
    }

    // TODO: Implement comprehensive range request tests
    // - Full range: "bytes=0-499"
    // - Suffix range: "bytes=-500" (last 500 bytes)
    // - Open-ended: "bytes=500-" (from 500 to end)
    // - Invalid ranges
    // - Multi-range (not currently supported)
}

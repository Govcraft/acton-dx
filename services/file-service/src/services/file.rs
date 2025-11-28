//! File service gRPC implementation.

use acton_dx_proto::file::v1::{
    file_service_server::FileService, DeleteRequest, DeleteResponse, DownloadRequest,
    DownloadResponse, FileMetadata, GetMetadataRequest, GetSignedUrlRequest, GetUrlRequest,
    GetUrlResponse, ListFilesRequest, ListFilesResponse, UploadRequest, UploadResponse,
};
use async_stream::try_stream;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info};

/// Internal error type to avoid large error sizes.
#[derive(Debug)]
struct FileError {
    message: String,
}

impl FileError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn into_status(self) -> Status {
        Status::internal(self.message)
    }
}

/// File service implementation.
pub struct FileServiceImpl {
    /// Base path for file storage.
    base_path: PathBuf,
    /// In-memory metadata store.
    metadata: Arc<RwLock<HashMap<String, StoredMetadata>>>,
    /// Public base URL for file access.
    public_base_url: String,
    /// Signing key for signed URLs.
    signing_key: Option<String>,
    /// Chunk size for streaming.
    chunk_size: usize,
}

/// Stored file metadata.
#[derive(Debug, Clone)]
struct StoredMetadata {
    id: String,
    filename: String,
    content_type: String,
    size: i64,
    checksum: String,
    created_at: i64,
    updated_at: i64,
    path: PathBuf,
    custom_metadata: HashMap<String, String>,
}

impl StoredMetadata {
    fn to_proto(&self) -> FileMetadata {
        FileMetadata {
            id: self.id.clone(),
            filename: self.filename.clone(),
            content_type: self.content_type.clone(),
            size: self.size,
            checksum: self.checksum.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            metadata: self.custom_metadata.clone(),
        }
    }
}

impl FileServiceImpl {
    /// Create a new file service.
    ///
    /// # Errors
    ///
    /// Returns error if base directory cannot be created.
    pub async fn new(
        base_path: PathBuf,
        public_base_url: String,
        signing_key: Option<String>,
        chunk_size: usize,
    ) -> anyhow::Result<Self> {
        // Ensure base directory exists
        fs::create_dir_all(&base_path).await?;

        info!(path = %base_path.display(), "File storage initialized");

        Ok(Self {
            base_path,
            metadata: Arc::new(RwLock::new(HashMap::new())),
            public_base_url,
            signing_key,
            chunk_size,
        })
    }

    /// Get current unix timestamp.
    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    }

    /// Generate a unique file ID.
    fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Calculate SHA-256 checksum of data.
    fn calculate_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Get the storage path for a file ID.
    fn get_storage_path(&self, file_id: &str) -> PathBuf {
        // Use first 2 characters of ID for directory sharding
        let shard = &file_id[..2.min(file_id.len())];
        self.base_path.join(shard).join(file_id)
    }

    /// Process upload from stream.
    async fn process_upload(
        &self,
        mut stream: Streaming<UploadRequest>,
    ) -> Result<StoredMetadata, FileError> {
        // First message should be metadata
        let first_msg = stream
            .message()
            .await
            .map_err(|e| FileError::new(format!("Stream error: {e}")))?
            .ok_or_else(|| FileError::new("Empty upload stream"))?;

        let Some(acton_dx_proto::file::v1::upload_request::Data::Metadata(upload_meta)) =
            first_msg.data
        else {
            return Err(FileError::new("First message must be metadata"));
        };

        let file_id = Self::generate_id();
        let storage_path = self.get_storage_path(&file_id);

        // Ensure parent directory exists
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| FileError::new(format!("Failed to create directory: {e}")))?;
        }

        // Collect all chunks and write to file
        let mut file_data = Vec::new();
        while let Some(msg) = stream
            .message()
            .await
            .map_err(|e| FileError::new(format!("Stream error: {e}")))?
        {
            if let Some(acton_dx_proto::file::v1::upload_request::Data::Chunk(chunk)) = msg.data {
                file_data.extend_from_slice(&chunk);
            }
        }

        // Write file
        let mut file = File::create(&storage_path)
            .await
            .map_err(|e| FileError::new(format!("Failed to create file: {e}")))?;

        file.write_all(&file_data)
            .await
            .map_err(|e| FileError::new(format!("Failed to write file: {e}")))?;

        let checksum = Self::calculate_checksum(&file_data);
        let now = Self::current_timestamp();
        let size = i64::try_from(file_data.len()).unwrap_or(i64::MAX);

        let stored = StoredMetadata {
            id: file_id,
            filename: upload_meta.filename,
            content_type: upload_meta.content_type,
            size,
            checksum,
            created_at: now,
            updated_at: now,
            path: storage_path,
            custom_metadata: upload_meta.metadata,
        };

        Ok(stored)
    }

    /// Generate signed URL.
    fn generate_signed_url(&self, file_id: &str, expires_at: i64) -> Result<String, FileError> {
        let key = self
            .signing_key
            .as_ref()
            .ok_or_else(|| FileError::new("Signing key not configured"))?;

        // Create signature: SHA256(key + file_id + expires_at)
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(file_id.as_bytes());
        hasher.update(expires_at.to_be_bytes());
        let signature = format!("{:x}", hasher.finalize());

        Ok(format!(
            "{}/{}?expires={}&sig={}",
            self.public_base_url, file_id, expires_at, signature
        ))
    }
}

type DownloadStream = Pin<Box<dyn Stream<Item = Result<DownloadResponse, Status>> + Send>>;

#[tonic::async_trait]
impl FileService for FileServiceImpl {
    type DownloadStream = DownloadStream;

    async fn upload(
        &self,
        request: Request<Streaming<UploadRequest>>,
    ) -> Result<Response<UploadResponse>, Status> {
        let stream = request.into_inner();

        match self.process_upload(stream).await {
            Ok(stored) => {
                let proto_meta = stored.to_proto();

                // Store metadata
                let mut metadata = self.metadata.write().await;
                metadata.insert(stored.id.clone(), stored);
                drop(metadata);

                debug!(id = %proto_meta.id, "File uploaded successfully");

                Ok(Response::new(UploadResponse {
                    success: true,
                    file: Some(proto_meta),
                    error: None,
                }))
            }
            Err(e) => {
                error!(error = %e.message, "Upload failed");
                Ok(Response::new(UploadResponse {
                    success: false,
                    file: None,
                    error: Some(e.message),
                }))
            }
        }
    }

    async fn download(
        &self,
        request: Request<DownloadRequest>,
    ) -> Result<Response<Self::DownloadStream>, Status> {
        let req = request.into_inner();
        debug!(file_id = %req.file_id, "Download request");

        let metadata_guard = self.metadata.read().await;
        let stored = metadata_guard
            .get(&req.file_id)
            .cloned()
            .ok_or_else(|| Status::not_found("File not found"))?;
        drop(metadata_guard);

        let chunk_size = self.chunk_size;
        let range_start = req.range_start.map(|v| u64::try_from(v).unwrap_or(0));
        let range_end = req.range_end.map(|v| u64::try_from(v).unwrap_or(u64::MAX));

        let output_stream = try_stream! {
            // First yield metadata
            yield DownloadResponse {
                data: Some(acton_dx_proto::file::v1::download_response::Data::Metadata(
                    stored.to_proto()
                )),
            };

            // Then yield file chunks
            let mut file = File::open(&stored.path).await.map_err(|e| {
                Status::internal(format!("Failed to open file: {e}"))
            })?;

            // Handle range requests
            if let Some(start) = range_start {
                use tokio::io::AsyncSeekExt;
                file.seek(std::io::SeekFrom::Start(start)).await.map_err(|e| {
                    Status::internal(format!("Failed to seek: {e}"))
                })?;
            }

            let mut buffer = vec![0u8; chunk_size];
            let mut total_read: u64 = 0;
            let max_read = range_end.map(|end| end - range_start.unwrap_or(0));

            loop {
                let to_read = max_read.map_or(chunk_size, |max| {
                    let remaining = max.saturating_sub(total_read);
                    chunk_size.min(usize::try_from(remaining).unwrap_or(chunk_size))
                });

                if to_read == 0 {
                    break;
                }

                let bytes_read = file.read(&mut buffer[..to_read]).await.map_err(|e| {
                    Status::internal(format!("Failed to read file: {e}"))
                })?;

                if bytes_read == 0 {
                    break;
                }

                total_read += u64::try_from(bytes_read).unwrap_or(0);

                yield DownloadResponse {
                    data: Some(acton_dx_proto::file::v1::download_response::Data::Chunk(
                        buffer[..bytes_read].to_vec()
                    )),
                };
            }
        };

        Ok(Response::new(Box::pin(output_stream)))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        debug!(file_id = %req.file_id, "Delete request");

        let mut metadata = self.metadata.write().await;
        let stored = metadata.remove(&req.file_id);
        drop(metadata);

        if let Some(stored) = stored {
            // Delete the actual file
            if let Err(e) = fs::remove_file(&stored.path).await {
                error!(error = %e, path = %stored.path.display(), "Failed to delete file");
            }

            info!(id = %req.file_id, "File deleted");
            Ok(Response::new(DeleteResponse { success: true }))
        } else {
            Ok(Response::new(DeleteResponse { success: false }))
        }
    }

    async fn get_metadata(
        &self,
        request: Request<GetMetadataRequest>,
    ) -> Result<Response<FileMetadata>, Status> {
        let req = request.into_inner();
        debug!(file_id = %req.file_id, "GetMetadata request");

        let metadata = self.metadata.read().await;
        let stored = metadata
            .get(&req.file_id)
            .ok_or_else(|| Status::not_found("File not found"))?;

        let result = stored.to_proto();
        drop(metadata);

        Ok(Response::new(result))
    }

    async fn list_files(
        &self,
        request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let req = request.into_inner();
        debug!(prefix = ?req.path_prefix, limit = ?req.limit, "ListFiles request");

        let metadata = self.metadata.read().await;

        let mut files: Vec<FileMetadata> = metadata
            .values()
            .filter(|f| {
                req.path_prefix
                    .as_ref()
                    .is_none_or(|prefix| f.filename.starts_with(prefix))
            })
            .map(StoredMetadata::to_proto)
            .collect();

        drop(metadata);

        // Sort by created_at descending
        files.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply limit with safe conversion
        let limit = usize::try_from(req.limit.unwrap_or(100)).unwrap_or(100);
        files.truncate(limit);

        Ok(Response::new(ListFilesResponse {
            files,
            next_cursor: None,
        }))
    }

    async fn get_public_url(
        &self,
        request: Request<GetUrlRequest>,
    ) -> Result<Response<GetUrlResponse>, Status> {
        let req = request.into_inner();
        debug!(file_id = %req.file_id, "GetPublicUrl request");

        // Verify file exists
        let metadata = self.metadata.read().await;
        if !metadata.contains_key(&req.file_id) {
            return Err(Status::not_found("File not found"));
        }
        drop(metadata);

        let url = format!("{}/{}", self.public_base_url, req.file_id);

        Ok(Response::new(GetUrlResponse {
            url,
            expires_at: None,
        }))
    }

    async fn get_signed_url(
        &self,
        request: Request<GetSignedUrlRequest>,
    ) -> Result<Response<GetUrlResponse>, Status> {
        let req = request.into_inner();
        debug!(file_id = %req.file_id, expires_in = req.expires_in_seconds, "GetSignedUrl request");

        // Verify file exists
        let metadata = self.metadata.read().await;
        if !metadata.contains_key(&req.file_id) {
            return Err(Status::not_found("File not found"));
        }
        drop(metadata);

        let expires_at = Self::current_timestamp() + req.expires_in_seconds;
        let url = self
            .generate_signed_url(&req.file_id, expires_at)
            .map_err(FileError::into_status)?;

        Ok(Response::new(GetUrlResponse {
            url,
            expires_at: Some(expires_at),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = FileServiceImpl::generate_id();
        let id2 = FileServiceImpl::generate_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 36); // UUID format
    }

    #[test]
    fn test_calculate_checksum() {
        let data = b"hello world";
        let checksum = FileServiceImpl::calculate_checksum(data);
        assert!(!checksum.is_empty());
        // SHA-256 produces 64 hex characters
        assert_eq!(checksum.len(), 64);
    }

    #[test]
    fn test_current_timestamp() {
        let ts = FileServiceImpl::current_timestamp();
        assert!(ts > 0);
    }
}

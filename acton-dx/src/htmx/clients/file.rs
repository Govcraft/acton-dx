//! File service client for file storage operations.

use super::error::ClientError;
use acton_dx_proto::file::v1::{
    file_service_client::FileServiceClient, DeleteRequest, DownloadRequest, FileMetadata,
    GetMetadataRequest, GetSignedUrlRequest, GetUrlRequest, ListFilesRequest, UploadMetadata,
    UploadRequest,
};
use futures_util::StreamExt;
use std::collections::HashMap;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;

/// Client for the file service.
///
/// Provides file storage operations including streaming uploads/downloads,
/// metadata management, and URL generation.
#[derive(Debug, Clone)]
pub struct FileClient {
    client: FileServiceClient<Channel>,
    chunk_size: usize,
}

impl FileClient {
    /// Connect to the file service.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, ClientError> {
        Self::connect_with_chunk_size(endpoint, 64 * 1024).await
    }

    /// Connect to the file service with a custom chunk size.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect_with_chunk_size(
        endpoint: impl Into<String>,
        chunk_size: usize,
    ) -> Result<Self, ClientError> {
        let endpoint = endpoint.into();
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| ClientError::ConnectionFailed(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            client: FileServiceClient::new(channel),
            chunk_size,
        })
    }

    /// Upload a file.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn upload(
        &mut self,
        filename: &str,
        content_type: &str,
        data: Vec<u8>,
        metadata: HashMap<String, String>,
    ) -> Result<UploadResult, ClientError> {
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let chunk_size = self.chunk_size;

        // Send metadata first
        let metadata_msg = UploadRequest {
            data: Some(acton_dx_proto::file::v1::upload_request::Data::Metadata(
                UploadMetadata {
                    filename: filename.to_string(),
                    content_type: content_type.to_string(),
                    path: None,
                    metadata,
                },
            )),
        };

        tx.send(metadata_msg)
            .await
            .map_err(|e| ClientError::RequestFailed(e.to_string()))?;

        // Send data chunks
        for chunk in data.chunks(chunk_size) {
            let chunk_msg = UploadRequest {
                data: Some(acton_dx_proto::file::v1::upload_request::Data::Chunk(
                    chunk.to_vec(),
                )),
            };
            tx.send(chunk_msg)
                .await
                .map_err(|e| ClientError::RequestFailed(e.to_string()))?;
        }

        // Close the channel
        drop(tx);

        let stream = ReceiverStream::new(rx);
        let response = self.client.upload(stream).await?;

        let inner = response.into_inner();
        if inner.success {
            Ok(UploadResult {
                success: true,
                file: inner.file.map(Into::into),
                error: None,
            })
        } else {
            Ok(UploadResult {
                success: false,
                file: None,
                error: inner.error,
            })
        }
    }

    /// Download a file.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn download(&mut self, file_id: &str) -> Result<DownloadResult, ClientError> {
        self.download_range(file_id, None, None).await
    }

    /// Download a range of a file.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn download_range(
        &mut self,
        file_id: &str,
        range_start: Option<i64>,
        range_end: Option<i64>,
    ) -> Result<DownloadResult, ClientError> {
        let response = self
            .client
            .download(DownloadRequest {
                file_id: file_id.to_string(),
                range_start,
                range_end,
            })
            .await?;

        let mut stream = response.into_inner();
        let mut metadata: Option<StoredFileInfo> = None;
        let mut data = Vec::new();

        while let Some(msg) = stream.next().await {
            let msg = msg?;
            match msg.data {
                Some(acton_dx_proto::file::v1::download_response::Data::Metadata(m)) => {
                    metadata = Some(m.into());
                }
                Some(acton_dx_proto::file::v1::download_response::Data::Chunk(chunk)) => {
                    data.extend_from_slice(&chunk);
                }
                None => {}
            }
        }

        Ok(DownloadResult {
            metadata: metadata.ok_or_else(|| {
                ClientError::ResponseError("No metadata in response".to_string())
            })?,
            data,
        })
    }

    /// Delete a file.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn delete(&mut self, file_id: &str) -> Result<bool, ClientError> {
        let response = self
            .client
            .delete(DeleteRequest {
                file_id: file_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().success)
    }

    /// Get file metadata.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn get_metadata(&mut self, file_id: &str) -> Result<StoredFileInfo, ClientError> {
        let response = self
            .client
            .get_metadata(GetMetadataRequest {
                file_id: file_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().into())
    }

    /// List files.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn list_files(
        &mut self,
        path_prefix: Option<String>,
        limit: Option<i32>,
        cursor: Option<String>,
    ) -> Result<ListResult, ClientError> {
        let response = self
            .client
            .list_files(ListFilesRequest {
                path_prefix,
                limit,
                cursor,
            })
            .await?;

        let inner = response.into_inner();
        Ok(ListResult {
            files: inner.files.into_iter().map(Into::into).collect(),
            next_cursor: inner.next_cursor,
        })
    }

    /// Get a public URL for a file.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn get_public_url(&mut self, file_id: &str) -> Result<String, ClientError> {
        let response = self
            .client
            .get_public_url(GetUrlRequest {
                file_id: file_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().url)
    }

    /// Get a signed URL for a file.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn get_signed_url(
        &mut self,
        file_id: &str,
        expires_in_seconds: i64,
    ) -> Result<SignedUrlResult, ClientError> {
        let response = self
            .client
            .get_signed_url(GetSignedUrlRequest {
                file_id: file_id.to_string(),
                expires_in_seconds,
            })
            .await?;

        let inner = response.into_inner();
        Ok(SignedUrlResult {
            url: inner.url,
            expires_at: inner.expires_at,
        })
    }
}

/// Result of an upload operation.
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// Whether the upload succeeded.
    pub success: bool,
    /// File metadata if successful.
    pub file: Option<StoredFileInfo>,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Result of a download operation.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// File metadata.
    pub metadata: StoredFileInfo,
    /// File data.
    pub data: Vec<u8>,
}

/// Stored file information.
#[derive(Debug, Clone)]
pub struct StoredFileInfo {
    /// File ID.
    pub id: String,
    /// Original filename.
    pub filename: String,
    /// MIME content type.
    pub content_type: String,
    /// File size in bytes.
    pub size: i64,
    /// SHA-256 checksum.
    pub checksum: String,
    /// Creation timestamp.
    pub created_at: i64,
    /// Last update timestamp.
    pub updated_at: i64,
    /// Custom metadata.
    pub metadata: HashMap<String, String>,
}

impl From<FileMetadata> for StoredFileInfo {
    fn from(m: FileMetadata) -> Self {
        Self {
            id: m.id,
            filename: m.filename,
            content_type: m.content_type,
            size: m.size,
            checksum: m.checksum,
            created_at: m.created_at,
            updated_at: m.updated_at,
            metadata: m.metadata,
        }
    }
}

/// Result of a list operation.
#[derive(Debug, Clone)]
pub struct ListResult {
    /// Files in the list.
    pub files: Vec<StoredFileInfo>,
    /// Cursor for next page.
    pub next_cursor: Option<String>,
}

/// Result of a signed URL request.
#[derive(Debug, Clone)]
pub struct SignedUrlResult {
    /// Signed URL.
    pub url: String,
    /// Expiration timestamp.
    pub expires_at: Option<i64>,
}

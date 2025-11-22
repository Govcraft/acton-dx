//! File storage abstraction and implementations
//!
//! This module provides a trait-based abstraction for file storage with multiple backends:
//! - Local filesystem storage (for development and small deployments)
//! - S3-compatible storage (AWS S3, MinIO, etc.) - planned for Week 8
//! - Azure Blob Storage - planned for Week 8
//!
//! # Architecture
//!
//! The `FileStorage` trait provides a consistent API for storing, retrieving, and managing files
//! regardless of the underlying storage backend. This allows applications to:
//! - Switch storage backends without changing handler code
//! - Test with local storage and deploy with S3
//! - Support multiple storage backends simultaneously
//!
//! # Examples
//!
//! ```rust,no_run
//! use acton_htmx::storage::{FileStorage, LocalFileStorage, UploadedFile, StoredFile};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create local storage backend
//! let storage = LocalFileStorage::new(PathBuf::from("/var/uploads"))?;
//!
//! // Store a file
//! let uploaded = UploadedFile {
//!     filename: "avatar.png".to_string(),
//!     content_type: "image/png".to_string(),
//!     data: vec![/* ... */],
//! };
//!
//! let stored = storage.store(uploaded).await?;
//! println!("Stored file: {}", stored.id);
//!
//! // Retrieve the file
//! let data = storage.retrieve(&stored.id).await?;
//!
//! // Delete the file
//! storage.delete(&stored.id).await?;
//! # Ok(())
//! # }
//! ```

mod local;
pub mod policy;
pub mod processing;
pub mod scanning;
mod traits;
mod types;
pub mod validation;

pub use local::LocalFileStorage;
pub use policy::{PolicyBuilder, UploadPolicy};
pub use processing::ImageProcessor;
pub use scanning::{ClamAvScanner, NoOpScanner, QuarantineScanner, ScanResult, VirusScanner};
#[cfg(feature = "clamav")]
pub use scanning::ClamAvConnection;
pub use traits::FileStorage;
pub use types::{StorageError, StorageResult, StoredFile, UploadedFile};
pub use validation::MimeValidator;

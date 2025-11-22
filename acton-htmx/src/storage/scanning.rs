//! Virus scanning integration for uploaded files
//!
//! This module provides a trait-based abstraction for virus scanning with
//! support for multiple backends like ClamAV.
//!
//! # Security Warning
//!
//! Virus scanning is an important defense-in-depth measure, but should not be
//! your only line of defense. Always combine virus scanning with:
//! - MIME type validation (magic number checking)
//! - File size limits
//! - Sandboxing/isolation of uploaded files
//! - Principle of least privilege
//!
//! # Examples
//!
//! ```rust,no_run
//! use acton_htmx::storage::{UploadedFile, scanning::{VirusScanner, NoOpScanner}};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let file = UploadedFile::new("document.pdf", "application/pdf", vec![/* ... */]);
//!
//! // Use a no-op scanner (for development/testing)
//! let scanner = NoOpScanner::new();
//! let result = scanner.scan(&file).await?;
//!
//! match result {
//!     ScanResult::Clean => println!("File is safe"),
//!     ScanResult::Infected { threat } => println!("File infected with: {}", threat),
//!     ScanResult::Error { message } => println!("Scan error: {}", message),
//! }
//! # Ok(())
//! # }
//! ```

use super::types::{StorageError, StorageResult, UploadedFile};
use async_trait::async_trait;
use std::fmt;

/// Result of a virus scan
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanResult {
    /// File is clean (no threats detected)
    Clean,

    /// File is infected
    Infected {
        /// Name/description of detected threat
        threat: String,
    },

    /// Scanning encountered an error
    Error {
        /// Error message
        message: String,
    },
}

impl fmt::Display for ScanResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean => write!(f, "Clean"),
            Self::Infected { threat } => write!(f, "Infected: {threat}"),
            Self::Error { message } => write!(f, "Scan error: {message}"),
        }
    }
}

/// Trait for virus scanning implementations
///
/// This trait allows for multiple virus scanning backends (ClamAV, Windows Defender,
/// cloud scanning services, etc.) with a consistent API.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait VirusScanner: Send + Sync {
    /// Scans a file for viruses and malware
    ///
    /// # Errors
    ///
    /// Returns error if scanning fails (e.g., scanner unavailable)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use acton_htmx::storage::{UploadedFile, scanning::{VirusScanner, NoOpScanner}};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let file = UploadedFile::new("test.pdf", "application/pdf", vec![]);
    /// let scanner = NoOpScanner::new();
    /// let result = scanner.scan(&file).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn scan(&self, file: &UploadedFile) -> StorageResult<ScanResult>;

    /// Returns the name of the scanner implementation
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::scanning::{VirusScanner, NoOpScanner};
    ///
    /// let scanner = NoOpScanner::new();
    /// assert_eq!(scanner.name(), "NoOp Scanner");
    /// ```
    fn name(&self) -> &'static str;

    /// Checks if the scanner is available and functional
    ///
    /// # Examples
    ///
    /// ```rust
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use acton_htmx::storage::scanning::{VirusScanner, NoOpScanner};
    ///
    /// let scanner = NoOpScanner::new();
    /// assert!(scanner.is_available().await);
    /// # Ok(())
    /// # }
    /// ```
    async fn is_available(&self) -> bool;
}

/// No-op scanner that always returns Clean
///
/// This scanner is useful for:
/// - Development and testing environments
/// - Deployments where virus scanning is handled externally
/// - Minimal overhead when scanning is not required
///
/// # Examples
///
/// ```rust
/// use acton_htmx::storage::scanning::{VirusScanner, NoOpScanner};
///
/// let scanner = NoOpScanner::new();
/// assert!(scanner.is_development_only());
/// ```
#[derive(Debug, Clone, Default)]
pub struct NoOpScanner;

impl NoOpScanner {
    /// Creates a new no-op scanner
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::scanning::NoOpScanner;
    ///
    /// let scanner = NoOpScanner::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns true (this is for development only)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::scanning::NoOpScanner;
    ///
    /// let scanner = NoOpScanner::new();
    /// assert!(scanner.is_development_only());
    /// ```
    #[must_use]
    pub const fn is_development_only(&self) -> bool {
        true
    }
}

#[async_trait]
impl VirusScanner for NoOpScanner {
    async fn scan(&self, _file: &UploadedFile) -> StorageResult<ScanResult> {
        // Always return Clean in development mode
        Ok(ScanResult::Clean)
    }

    fn name(&self) -> &'static str {
        "NoOp Scanner"
    }

    async fn is_available(&self) -> bool {
        true
    }
}

/// ClamAV connection type
///
/// Specifies how to connect to the ClamAV daemon (clamd).
#[cfg(feature = "clamav")]
#[derive(Debug, Clone)]
pub enum ClamAvConnection {
    /// TCP connection
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "clamav")]
    /// # {
    /// use acton_htmx::storage::scanning::{ClamAvScanner, ClamAvConnection};
    ///
    /// let scanner = ClamAvScanner::new(ClamAvConnection::Tcp {
    ///     host: "localhost".to_string(),
    ///     port: 3310,
    /// });
    /// # }
    /// ```
    Tcp {
        /// Hostname or IP address
        host: String,
        /// Port number
        port: u16,
    },

    /// Unix domain socket connection
    ///
    /// Only available on Unix platforms (Linux, macOS, etc.)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature = "clamav", unix))]
    /// # {
    /// use acton_htmx::storage::scanning::{ClamAvScanner, ClamAvConnection};
    /// use std::path::PathBuf;
    ///
    /// let scanner = ClamAvScanner::new(ClamAvConnection::Socket {
    ///     path: PathBuf::from("/var/run/clamav/clamd.sock"),
    /// });
    /// # }
    /// ```
    #[cfg(unix)]
    Socket {
        /// Path to Unix domain socket
        path: std::path::PathBuf,
    },
}

/// ClamAV virus scanner
///
/// Integrates with ClamAV daemon (clamd) for virus scanning. Supports both
/// TCP and Unix socket connections.
///
/// # Feature Flag
///
/// This scanner requires the `clamav` feature to be enabled:
///
/// ```toml
/// [dependencies]
/// acton-htmx = { version = "1.0", features = ["clamav"] }
/// ```
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(feature = "clamav")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use acton_htmx::storage::{UploadedFile, scanning::{VirusScanner, ClamAvScanner, ClamAvConnection}};
///
/// let scanner = ClamAvScanner::new(ClamAvConnection::Tcp {
///     host: "localhost".to_string(),
///     port: 3310,
/// });
///
/// // Check if ClamAV is available
/// if !scanner.is_available().await {
///     eprintln!("ClamAV daemon is not available");
///     return Ok(());
/// }
///
/// // Scan a file
/// let file = UploadedFile::new("document.pdf", "application/pdf", vec![/* ... */]);
/// let result = scanner.scan(&file).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "clamav")]
#[derive(Debug, Clone)]
pub struct ClamAvScanner {
    connection: ClamAvConnection,
}

#[cfg(feature = "clamav")]
impl ClamAvScanner {
    /// Creates a new ClamAV scanner
    ///
    /// # Arguments
    ///
    /// * `connection` - How to connect to the ClamAV daemon
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "clamav")]
    /// # {
    /// use acton_htmx::storage::scanning::{ClamAvScanner, ClamAvConnection};
    ///
    /// // TCP connection
    /// let scanner = ClamAvScanner::new(ClamAvConnection::Tcp {
    ///     host: "localhost".to_string(),
    ///     port: 3310,
    /// });
    ///
    /// // Unix socket connection
    /// let scanner = ClamAvScanner::new(ClamAvConnection::Socket {
    ///     path: "/var/run/clamav/clamd.sock".into(),
    /// });
    /// # }
    /// ```
    #[must_use]
    pub const fn new(connection: ClamAvConnection) -> Self {
        Self { connection }
    }

    /// Creates a new ClamAV scanner with default TCP settings
    ///
    /// Connects to `localhost:3310` (the default ClamAV port).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "clamav")]
    /// # {
    /// use acton_htmx::storage::scanning::ClamAvScanner;
    ///
    /// let scanner = ClamAvScanner::default_tcp();
    /// # }
    /// ```
    #[must_use]
    pub fn default_tcp() -> Self {
        Self::new(ClamAvConnection::Tcp {
            host: "localhost".to_string(),
            port: 3310,
        })
    }

    /// Creates a new ClamAV scanner with default Unix socket settings
    ///
    /// Connects to `/var/run/clamav/clamd.sock` (common default path).
    ///
    /// Only available on Unix platforms.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature = "clamav", unix))]
    /// # {
    /// use acton_htmx::storage::scanning::ClamAvScanner;
    ///
    /// let scanner = ClamAvScanner::default_socket();
    /// # }
    /// ```
    #[must_use]
    #[cfg(unix)]
    pub fn default_socket() -> Self {
        Self::new(ClamAvConnection::Socket {
            path: "/var/run/clamav/clamd.sock".into(),
        })
    }
}

#[cfg(feature = "clamav")]
#[async_trait]
impl VirusScanner for ClamAvScanner {
    async fn scan(&self, file: &UploadedFile) -> StorageResult<ScanResult> {
        use clamav_client::tokio::{scan_buffer, Tcp};
        #[cfg(unix)]
        use clamav_client::tokio::Socket;

        // Get file data (data is a field, not a method)
        let data = &file.data;

        // Scan based on connection type
        let response = match &self.connection {
            ClamAvConnection::Tcp { host, port } => {
                let host_address = format!("{}:{}", host, port);
                let clamd = Tcp {
                    host_address: &host_address,
                };
                scan_buffer(data, clamd, None)
                    .await
                    .map_err(|e| StorageError::Other(format!("ClamAV scan failed: {}", e)))?
            }
            #[cfg(unix)]
            ClamAvConnection::Socket { path } => {
                let path_str = path
                    .to_str()
                    .ok_or_else(|| StorageError::Other("Invalid socket path".to_string()))?;
                let clamd = Socket {
                    socket_path: path_str,
                };
                scan_buffer(data, clamd, None)
                    .await
                    .map_err(|e| StorageError::Other(format!("ClamAV scan failed: {}", e)))?
            }
            #[cfg(not(unix))]
            ClamAvConnection::Socket { .. } => {
                return Err(StorageError::Other(
                    "Unix socket connections not supported on this platform".to_string(),
                ))
            }
        };

        // Parse response
        match clamav_client::clean(&response) {
            Ok(true) => Ok(ScanResult::Clean),
            Ok(false) => {
                // Extract threat name from response
                let threat = String::from_utf8_lossy(&response).trim().to_string();
                Ok(ScanResult::Infected { threat })
            }
            Err(e) => Ok(ScanResult::Error {
                message: format!("Failed to parse scan result: {}", e),
            }),
        }
    }

    fn name(&self) -> &'static str {
        "ClamAV Scanner"
    }

    async fn is_available(&self) -> bool {
        use clamav_client::tokio::{ping, Tcp};
        use clamav_client::PONG;
        #[cfg(unix)]
        use clamav_client::tokio::Socket;

        match &self.connection {
            ClamAvConnection::Tcp { host, port } => {
                let host_address = format!("{}:{}", host, port);
                let clamd = Tcp {
                    host_address: &host_address,
                };
                matches!(ping(clamd).await, Ok(response) if response == *PONG)
            }
            #[cfg(unix)]
            ClamAvConnection::Socket { path } => {
                let Some(path_str) = path.to_str() else {
                    return false;
                };
                let clamd = Socket {
                    socket_path: path_str,
                };
                matches!(ping(clamd).await, Ok(response) if response == *PONG)
            }
            #[cfg(not(unix))]
            ClamAvConnection::Socket { .. } => false,
        }
    }
}

/// ClamAV scanner placeholder (when feature is disabled)
///
/// This is a compile-time placeholder that exists when the `clamav` feature
/// is not enabled. It always returns an error indicating that ClamAV support
/// is not compiled in.
///
/// # Examples
///
/// ```rust
/// # #[cfg(not(feature = "clamav"))]
/// # {
/// use acton_htmx::storage::scanning::ClamAvScanner;
///
/// let scanner = ClamAvScanner::new();
/// # }
/// ```
#[cfg(not(feature = "clamav"))]
#[derive(Debug, Clone, Default)]
pub struct ClamAvScanner;

#[cfg(not(feature = "clamav"))]
impl ClamAvScanner {
    /// Creates a new ClamAV scanner placeholder
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(not(feature = "clamav"))]
    /// # {
    /// use acton_htmx::storage::scanning::ClamAvScanner;
    ///
    /// let scanner = ClamAvScanner::new();
    /// # }
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "clamav"))]
#[async_trait]
impl VirusScanner for ClamAvScanner {
    async fn scan(&self, _file: &UploadedFile) -> StorageResult<ScanResult> {
        Err(StorageError::Other(
            "ClamAV support not enabled. Recompile with 'clamav' feature.".to_string(),
        ))
    }

    fn name(&self) -> &'static str {
        "ClamAV Scanner (disabled)"
    }

    async fn is_available(&self) -> bool {
        false
    }
}

/// Scanner that quarantines infected files
///
/// This wrapper scanner wraps another scanner and automatically quarantines
/// files that are detected as infected.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::storage::scanning::{QuarantineScanner, NoOpScanner};
/// use std::path::PathBuf;
///
/// let base_scanner = NoOpScanner::new();
/// let scanner = QuarantineScanner::new(
///     base_scanner,
///     PathBuf::from("/var/quarantine"),
/// );
/// ```
#[derive(Debug)]
pub struct QuarantineScanner<S: VirusScanner> {
    /// Underlying scanner
    inner: S,

    /// Path to quarantine directory
    #[allow(dead_code)]
    quarantine_path: std::path::PathBuf,
}

impl<S: VirusScanner> QuarantineScanner<S> {
    /// Creates a new quarantine scanner
    ///
    /// # Arguments
    ///
    /// * `scanner` - The underlying virus scanner
    /// * `quarantine_path` - Directory where infected files will be moved
    ///
    /// # Examples
    ///
    /// ```rust
    /// use acton_htmx::storage::scanning::{QuarantineScanner, NoOpScanner};
    /// use std::path::PathBuf;
    ///
    /// let scanner = QuarantineScanner::new(
    ///     NoOpScanner::new(),
    ///     PathBuf::from("/var/quarantine"),
    /// );
    /// ```
    #[must_use]
    pub const fn new(scanner: S, quarantine_path: std::path::PathBuf) -> Self {
        Self {
            inner: scanner,
            quarantine_path,
        }
    }
}

#[async_trait]
impl<S: VirusScanner> VirusScanner for QuarantineScanner<S> {
    async fn scan(&self, file: &UploadedFile) -> StorageResult<ScanResult> {
        let result = self.inner.scan(file).await?;

        if let ScanResult::Infected { .. } = result {
            // TODO: Implement quarantine logic
            // 1. Create quarantine directory if it doesn't exist
            // 2. Generate unique filename in quarantine
            // 3. Write file to quarantine with metadata (timestamp, threat name, original path)
            // 4. Optionally encrypt quarantined file
            // 5. Log quarantine event
        }

        Ok(result)
    }

    fn name(&self) -> &'static str {
        "Quarantine Scanner"
    }

    async fn is_available(&self) -> bool {
        self.inner.is_available().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_scanner_always_clean() {
        let file = UploadedFile::new("test.txt", "text/plain", b"harmless data".to_vec());
        let scanner = NoOpScanner::new();

        let result = scanner.scan(&file).await.unwrap();
        assert_eq!(result, ScanResult::Clean);
    }

    #[tokio::test]
    async fn test_noop_scanner_available() {
        let scanner = NoOpScanner::new();
        assert!(scanner.is_available().await);
    }

    #[tokio::test]
    async fn test_noop_scanner_name() {
        let scanner = NoOpScanner::new();
        assert_eq!(scanner.name(), "NoOp Scanner");
    }

    #[cfg(feature = "clamav")]
    #[tokio::test]
    async fn test_clamav_scanner_tcp_not_available() {
        let scanner = ClamAvScanner::new(ClamAvConnection::Tcp {
            host: "nonexistent.invalid".to_string(),
            port: 9999,
        });
        assert!(!scanner.is_available().await);
    }

    #[cfg(all(feature = "clamav", unix))]
    #[tokio::test]
    async fn test_clamav_scanner_socket_not_available() {
        let scanner = ClamAvScanner::new(ClamAvConnection::Socket {
            path: "/nonexistent/path.sock".into(),
        });
        assert!(!scanner.is_available().await);
    }

    #[cfg(feature = "clamav")]
    #[tokio::test]
    async fn test_clamav_scanner_default_tcp() {
        let scanner = ClamAvScanner::default_tcp();
        assert_eq!(scanner.name(), "ClamAV Scanner");
    }

    #[cfg(all(feature = "clamav", unix))]
    #[tokio::test]
    async fn test_clamav_scanner_default_socket() {
        let scanner = ClamAvScanner::default_socket();
        assert_eq!(scanner.name(), "ClamAV Scanner");
    }

    #[cfg(feature = "clamav")]
    #[tokio::test]
    async fn test_clamav_scanner_scan_connection_refused() {
        let file = UploadedFile::new("test.txt", "text/plain", b"test data".to_vec());
        let scanner = ClamAvScanner::new(ClamAvConnection::Tcp {
            host: "localhost".to_string(),
            port: 9999, // Non-existent port
        });

        let result = scanner.scan(&file).await;
        // Should fail with connection error
        assert!(result.is_err());
        if let Err(StorageError::Other(msg)) = result {
            assert!(msg.contains("ClamAV scan failed"));
        }
    }

    #[cfg(not(feature = "clamav"))]
    #[tokio::test]
    async fn test_clamav_scanner_disabled() {
        let file = UploadedFile::new("test.txt", "text/plain", b"test data".to_vec());
        let scanner = ClamAvScanner::new();

        let result = scanner.scan(&file).await;
        assert!(result.is_err());
        if let Err(StorageError::Other(msg)) = result {
            assert!(msg.contains("not enabled"));
        }
    }

    #[cfg(not(feature = "clamav"))]
    #[tokio::test]
    async fn test_clamav_scanner_disabled_not_available() {
        let scanner = ClamAvScanner::new();
        assert!(!scanner.is_available().await);
        assert_eq!(scanner.name(), "ClamAV Scanner (disabled)");
    }

    #[test]
    fn test_scan_result_display() {
        assert_eq!(ScanResult::Clean.to_string(), "Clean");
        assert_eq!(
            ScanResult::Infected {
                threat: "EICAR".to_string()
            }
            .to_string(),
            "Infected: EICAR"
        );
        assert_eq!(
            ScanResult::Error {
                message: "Scanner offline".to_string()
            }
            .to_string(),
            "Scan error: Scanner offline"
        );
    }

    #[tokio::test]
    async fn test_quarantine_scanner_wraps_inner() {
        let file = UploadedFile::new("test.txt", "text/plain", b"test".to_vec());
        let scanner = QuarantineScanner::new(
            NoOpScanner::new(),
            std::path::PathBuf::from("/tmp/quarantine"),
        );

        let result = scanner.scan(&file).await.unwrap();
        assert_eq!(result, ScanResult::Clean);
    }
}

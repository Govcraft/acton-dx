//! IPC (Inter-Process Communication) client implementation.
//!
//! This module provides IPC-based clients for communicating with Acton DX
//! microservices using Unix Domain Sockets via the acton-reactive IPC system.
//!
//! # Architecture
//!
//! The IPC client uses acton-reactive's protocol v2 which provides:
//! - JSON or MessagePack serialization
//! - Request-response pattern with correlation IDs
//! - Push notifications for subscriptions
//! - Service discovery
//! - Rate limiting and backpressure handling
//!
//! # Wire Protocol
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ Frame Length (4 bytes, big-endian u32)                      │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Protocol Version (1 byte, 0x02)                             │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Message Type (1 byte: Request=0x01, Response=0x02, etc.)    │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Format (1 byte: JSON=0x01, MessagePack=0x02)                │
//! ├─────────────────────────────────────────────────────────────┤
//! │ Payload (JSON or MessagePack encoded)                       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use acton_dx::htmx::clients::ipc::{IpcClient, IpcClientConfig};
//!
//! let config = IpcClientConfig::default();
//! let mut client = IpcClient::connect(config).await?;
//!
//! // Send a request to the auth service
//! let response = client.request(
//!     "auth_service",
//!     "CreateSession",
//!     serde_json::json!({
//!         "ttl_seconds": 3600,
//!         "initial_data": {}
//!     }),
//! ).await?;
//! ```

use super::error::ClientError;
use super::transport::IpcTransportConfig;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

/// Protocol version for IPC communication.
const PROTOCOL_VERSION: u8 = 0x02;

/// Message type constants.
mod msg_type {
    pub const REQUEST: u8 = 0x01;
    pub const RESPONSE: u8 = 0x02;
    pub const ERROR: u8 = 0x03;
    pub const HEARTBEAT: u8 = 0x04;
    pub const PUSH: u8 = 0x05;
    pub const SUBSCRIBE: u8 = 0x06;
    pub const UNSUBSCRIBE: u8 = 0x07;
    pub const DISCOVERY: u8 = 0x08;
}

/// Serialization format.
mod format {
    pub const JSON: u8 = 0x01;
    #[allow(dead_code)]
    pub const MESSAGEPACK: u8 = 0x02;
}

/// IPC request envelope sent to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEnvelope {
    /// Unique request ID for correlation.
    pub correlation_id: String,
    /// Target actor name or ERN.
    pub target: String,
    /// Message type name for deserialization.
    pub message_type: String,
    /// Serialized message payload.
    pub payload: serde_json::Value,
    /// Whether a reply is expected.
    #[serde(default)]
    pub expects_reply: bool,
    /// Whether streaming response is expected.
    #[serde(default)]
    pub expects_stream: bool,
    /// Request timeout in milliseconds.
    #[serde(default = "default_timeout")]
    pub response_timeout_ms: u64,
}

fn default_timeout() -> u64 {
    30_000
}

impl IpcEnvelope {
    /// Create a new IPC envelope for fire-and-forget messages.
    pub fn new(target: impl Into<String>, message_type: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            correlation_id: generate_correlation_id(),
            target: target.into(),
            message_type: message_type.into(),
            payload,
            expects_reply: false,
            expects_stream: false,
            response_timeout_ms: default_timeout(),
        }
    }

    /// Create a new IPC envelope that expects a reply.
    pub fn new_request(
        target: impl Into<String>,
        message_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            correlation_id: generate_correlation_id(),
            target: target.into(),
            message_type: message_type.into(),
            payload,
            expects_reply: true,
            expects_stream: false,
            response_timeout_ms: default_timeout(),
        }
    }

    /// Set the response timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.response_timeout_ms = timeout_ms;
        self
    }
}

/// IPC response from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    /// Correlation ID matching the request.
    pub correlation_id: String,
    /// Whether the request was successful.
    pub success: bool,
    /// Error message if unsuccessful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Machine-readable error code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Response payload if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl IpcResponse {
    /// Extract the payload, converting to the expected type.
    ///
    /// # Errors
    ///
    /// Returns error if the response was unsuccessful or deserialization fails.
    pub fn extract<T: DeserializeOwned>(self) -> Result<T, ClientError> {
        if !self.success {
            return Err(ClientError::ServiceError {
                code: self.error_code.unwrap_or_else(|| "UNKNOWN".to_string()),
                message: self.error.unwrap_or_else(|| "Unknown error".to_string()),
            });
        }

        let payload = self
            .payload
            .ok_or_else(|| ClientError::ResponseError("No payload in response".to_string()))?;

        serde_json::from_value(payload)
            .map_err(|e| ClientError::DeserializationError(e.to_string()))
    }

    /// Check if the response indicates success.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.success
    }
}

/// Configuration for the IPC client.
#[derive(Debug, Clone)]
pub struct IpcClientConfig {
    /// Socket path to connect to.
    pub socket_path: PathBuf,
    /// Request timeout.
    pub timeout: Duration,
    /// Maximum retries for connection.
    pub max_retries: u32,
    /// Delay between retries.
    pub retry_delay: Duration,
    /// Maximum message size.
    pub max_message_size: usize,
}

impl Default for IpcClientConfig {
    fn default() -> Self {
        let transport_config = IpcTransportConfig::default();
        Self::from_transport_config(&transport_config)
    }
}

impl IpcClientConfig {
    /// Create config from transport configuration.
    #[must_use]
    pub fn from_transport_config(config: &IpcTransportConfig) -> Self {
        Self {
            socket_path: config.socket_path(),
            timeout: Duration::from_millis(config.timeout_ms),
            max_retries: config.max_retries,
            retry_delay: Duration::from_millis(config.retry_delay_ms),
            max_message_size: config.max_message_size,
        }
    }

    /// Create config with a specific socket path.
    #[must_use]
    pub fn with_socket_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.socket_path = path.into();
        self
    }
}

/// IPC client for communicating with services.
///
/// The client maintains a persistent connection to the IPC socket
/// and handles request/response correlation.
#[derive(Debug)]
pub struct IpcClient {
    config: IpcClientConfig,
    stream: Arc<Mutex<Option<UnixStream>>>,
    request_counter: AtomicU64,
}

impl IpcClient {
    /// Create a new IPC client with the given configuration.
    #[must_use]
    pub fn new(config: IpcClientConfig) -> Self {
        Self {
            config,
            stream: Arc::new(Mutex::new(None)),
            request_counter: AtomicU64::new(0),
        }
    }

    /// Connect to the IPC socket.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails after all retries.
    pub async fn connect(config: IpcClientConfig) -> Result<Self, ClientError> {
        let client = Self::new(config);
        client.ensure_connected().await?;
        Ok(client)
    }

    /// Ensure the client is connected, reconnecting if necessary.
    async fn ensure_connected(&self) -> Result<(), ClientError> {
        let mut stream_guard = self.stream.lock().await;

        if stream_guard.is_some() {
            return Ok(());
        }

        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tokio::time::sleep(self.config.retry_delay).await;
            }

            match UnixStream::connect(&self.config.socket_path).await {
                Ok(stream) => {
                    tracing::debug!(
                        socket = %self.config.socket_path.display(),
                        "IPC client connected"
                    );
                    *stream_guard = Some(stream);
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries = self.config.max_retries,
                        error = %e,
                        "IPC connection attempt failed"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(ClientError::ConnectionFailed(
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }

    /// Send a fire-and-forget message to a service.
    ///
    /// # Errors
    ///
    /// Returns error if connection or sending fails.
    pub async fn send(
        &self,
        target: &str,
        message_type: &str,
        payload: serde_json::Value,
    ) -> Result<(), ClientError> {
        self.ensure_connected().await?;

        let envelope = IpcEnvelope::new(target, message_type, payload);

        let mut stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_mut()
            .ok_or_else(|| ClientError::ConnectionFailed("Not connected".to_string()))?;

        write_frame(stream, msg_type::REQUEST, &envelope).await?;

        Ok(())
    }

    /// Send a request and wait for a response.
    ///
    /// # Errors
    ///
    /// Returns error if connection, sending, or receiving fails.
    pub async fn request(
        &self,
        target: &str,
        message_type: &str,
        payload: serde_json::Value,
    ) -> Result<IpcResponse, ClientError> {
        self.ensure_connected().await?;

        let envelope = IpcEnvelope::new_request(target, message_type, payload)
            .with_timeout(self.config.timeout.as_millis() as u64);

        let mut stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_mut()
            .ok_or_else(|| ClientError::ConnectionFailed("Not connected".to_string()))?;

        // Send request
        write_frame(stream, msg_type::REQUEST, &envelope).await?;

        // Read response with timeout
        let response = tokio::time::timeout(self.config.timeout, async {
            read_response(stream, self.config.max_message_size).await
        })
        .await
        .map_err(|_| ClientError::Timeout)??;

        Ok(response)
    }

    /// Send a typed request and deserialize the response.
    ///
    /// # Errors
    ///
    /// Returns error if request fails or response cannot be deserialized.
    pub async fn request_typed<T: DeserializeOwned>(
        &self,
        target: &str,
        message_type: &str,
        payload: serde_json::Value,
    ) -> Result<T, ClientError> {
        let response = self.request(target, message_type, payload).await?;
        response.extract()
    }

    /// Close the connection.
    pub async fn close(&self) {
        let mut stream_guard = self.stream.lock().await;
        *stream_guard = None;
    }

    /// Check if the socket exists.
    #[must_use]
    pub fn socket_exists(&self) -> bool {
        self.config.socket_path.exists()
    }

    /// Generate a unique correlation ID for requests.
    fn next_correlation_id(&self) -> String {
        let counter = self.request_counter.fetch_add(1, Ordering::SeqCst);
        format!("req_{counter:016x}")
    }
}

// ============================================================================
// Wire Protocol Implementation
// ============================================================================

/// Write a framed message to the stream.
async fn write_frame<T: Serialize>(
    stream: &mut UnixStream,
    msg_type: u8,
    payload: &T,
) -> Result<(), ClientError> {
    let payload_bytes = serde_json::to_vec(payload)
        .map_err(|e| ClientError::SerializationError(e.to_string()))?;

    // Calculate frame length (excludes the 4-byte length field itself)
    let frame_len = 3 + payload_bytes.len(); // version + msg_type + format + payload

    // Build the frame
    let mut frame = Vec::with_capacity(4 + frame_len);

    // Frame length (4 bytes, big-endian)
    frame.extend_from_slice(&(frame_len as u32).to_be_bytes());

    // Protocol version (1 byte)
    frame.push(PROTOCOL_VERSION);

    // Message type (1 byte)
    frame.push(msg_type);

    // Format (1 byte)
    frame.push(format::JSON);

    // Payload
    frame.extend_from_slice(&payload_bytes);

    // Write the entire frame
    stream
        .write_all(&frame)
        .await
        .map_err(|e| ClientError::IoError(e.to_string()))?;

    Ok(())
}

/// Read a response frame from the stream.
async fn read_response(
    stream: &mut UnixStream,
    max_size: usize,
) -> Result<IpcResponse, ClientError> {
    // Read frame length (4 bytes)
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| ClientError::IoError(e.to_string()))?;

    let frame_len = u32::from_be_bytes(len_buf) as usize;

    if frame_len > max_size {
        return Err(ClientError::ResponseError(format!(
            "Frame too large: {frame_len} > {max_size}"
        )));
    }

    if frame_len < 3 {
        return Err(ClientError::ResponseError(
            "Frame too small".to_string(),
        ));
    }

    // Read the rest of the frame
    let mut frame = vec![0u8; frame_len];
    stream
        .read_exact(&mut frame)
        .await
        .map_err(|e| ClientError::IoError(e.to_string()))?;

    // Parse header
    let _version = frame[0];
    let msg_type = frame[1];
    let _format = frame[2];

    // Parse payload
    let payload_bytes = &frame[3..];

    match msg_type {
        msg_type::RESPONSE | msg_type::ERROR => {
            let response: IpcResponse = serde_json::from_slice(payload_bytes)
                .map_err(|e| ClientError::DeserializationError(e.to_string()))?;
            Ok(response)
        }
        _ => Err(ClientError::ResponseError(format!(
            "Unexpected message type: {msg_type}"
        ))),
    }
}

/// Generate a unique correlation ID.
fn generate_correlation_id() -> String {
    use std::sync::atomic::AtomicU64;
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    format!("req_{timestamp:x}_{counter:08x}")
}

// ============================================================================
// Service-Specific IPC Clients
// ============================================================================

/// IPC client for the auth service.
///
/// Provides session management, password hashing, CSRF tokens, and user operations
/// over IPC.
#[derive(Debug, Clone)]
pub struct IpcAuthClient {
    client: Arc<IpcClient>,
}

impl IpcAuthClient {
    /// Create a new auth client from a shared IPC client.
    #[must_use]
    pub fn new(client: Arc<IpcClient>) -> Self {
        Self { client }
    }

    /// Connect to the auth service.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(config: IpcClientConfig) -> Result<Self, ClientError> {
        let client = IpcClient::connect(config).await?;
        Ok(Self::new(Arc::new(client)))
    }

    // ==================== Session Operations ====================

    /// Create a new session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn create_session(
        &self,
        user_id: Option<i64>,
        ttl_seconds: i64,
        initial_data: HashMap<String, String>,
    ) -> Result<IpcSession, ClientError> {
        let payload = serde_json::json!({
            "user_id": user_id,
            "ttl_seconds": ttl_seconds,
            "initial_data": initial_data,
        });

        self.client
            .request_typed("auth_service", "CreateSession", payload)
            .await
    }

    /// Validate an existing session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn validate_session(&self, session_id: &str) -> Result<Option<IpcSession>, ClientError> {
        let payload = serde_json::json!({
            "session_id": session_id,
        });

        let response: ValidateSessionResponse = self
            .client
            .request_typed("auth_service", "ValidateSession", payload)
            .await?;

        if response.valid {
            Ok(response.session)
        } else {
            Ok(None)
        }
    }

    /// Update session data.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn update_session(
        &self,
        session_id: &str,
        data: HashMap<String, String>,
        user_id: Option<i64>,
    ) -> Result<Option<IpcSession>, ClientError> {
        let payload = serde_json::json!({
            "session_id": session_id,
            "data": data,
            "user_id": user_id,
        });

        let response: UpdateSessionResponse = self
            .client
            .request_typed("auth_service", "UpdateSession", payload)
            .await?;

        if response.success {
            Ok(response.session)
        } else {
            Ok(None)
        }
    }

    /// Destroy a session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn destroy_session(&self, session_id: &str) -> Result<bool, ClientError> {
        let payload = serde_json::json!({
            "session_id": session_id,
        });

        let response: DestroySessionResponse = self
            .client
            .request_typed("auth_service", "DestroySession", payload)
            .await?;

        Ok(response.success)
    }

    // ==================== Password Operations ====================

    /// Hash a password using Argon2.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn hash_password(&self, password: &str) -> Result<String, ClientError> {
        let payload = serde_json::json!({
            "password": password,
        });

        let response: HashPasswordResponse = self
            .client
            .request_typed("auth_service", "HashPassword", payload)
            .await?;

        Ok(response.hash)
    }

    /// Verify a password against a hash.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn verify_password(&self, password: &str, hash: &str) -> Result<bool, ClientError> {
        let payload = serde_json::json!({
            "password": password,
            "hash": hash,
        });

        let response: VerifyPasswordResponse = self
            .client
            .request_typed("auth_service", "VerifyPassword", payload)
            .await?;

        Ok(response.valid)
    }

    // ==================== CSRF Operations ====================

    /// Generate a CSRF token for a session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn generate_csrf_token(&self, session_id: &str) -> Result<String, ClientError> {
        let payload = serde_json::json!({
            "session_id": session_id,
        });

        let response: GenerateCsrfResponse = self
            .client
            .request_typed("auth_service", "GenerateCsrfToken", payload)
            .await?;

        Ok(response.token)
    }

    /// Validate a CSRF token.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn validate_csrf_token(&self, session_id: &str, token: &str) -> Result<bool, ClientError> {
        let payload = serde_json::json!({
            "session_id": session_id,
            "token": token,
        });

        let response: ValidateCsrfResponse = self
            .client
            .request_typed("auth_service", "ValidateCsrfToken", payload)
            .await?;

        Ok(response.valid)
    }
}

// ============================================================================
// IPC Response Types
// ============================================================================

/// Session data from IPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcSession {
    /// Unique session identifier.
    pub session_id: String,
    /// Associated user ID (if authenticated).
    pub user_id: Option<i64>,
    /// Session data as key-value pairs.
    pub data: HashMap<String, String>,
    /// Session creation timestamp (Unix ms).
    pub created_at: i64,
    /// Session expiration timestamp (Unix ms).
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
struct ValidateSessionResponse {
    valid: bool,
    session: Option<IpcSession>,
}

#[derive(Debug, Deserialize)]
struct UpdateSessionResponse {
    success: bool,
    session: Option<IpcSession>,
}

#[derive(Debug, Deserialize)]
struct DestroySessionResponse {
    success: bool,
}

#[derive(Debug, Deserialize)]
struct HashPasswordResponse {
    hash: String,
}

#[derive(Debug, Deserialize)]
struct VerifyPasswordResponse {
    valid: bool,
}

#[derive(Debug, Deserialize)]
struct GenerateCsrfResponse {
    token: String,
}

#[derive(Debug, Deserialize)]
struct ValidateCsrfResponse {
    valid: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_envelope_creation() {
        let envelope = IpcEnvelope::new(
            "auth_service",
            "CreateSession",
            serde_json::json!({"ttl_seconds": 3600}),
        );

        assert_eq!(envelope.target, "auth_service");
        assert_eq!(envelope.message_type, "CreateSession");
        assert!(!envelope.expects_reply);
        assert!(!envelope.correlation_id.is_empty());
    }

    #[test]
    fn test_ipc_envelope_request() {
        let envelope = IpcEnvelope::new_request(
            "auth_service",
            "ValidateSession",
            serde_json::json!({"session_id": "abc123"}),
        );

        assert!(envelope.expects_reply);
        assert_eq!(envelope.response_timeout_ms, 30_000);
    }

    #[test]
    fn test_ipc_response_extract_success() {
        let response = IpcResponse {
            correlation_id: "req_001".to_string(),
            success: true,
            error: None,
            error_code: None,
            payload: Some(serde_json::json!({"value": 42})),
        };

        #[derive(Debug, Deserialize, PartialEq)]
        struct TestPayload {
            value: i32,
        }

        let extracted: Result<TestPayload, _> = response.extract();
        assert!(extracted.is_ok());
        assert_eq!(extracted.unwrap().value, 42);
    }

    #[test]
    fn test_ipc_response_extract_error() {
        let response = IpcResponse {
            correlation_id: "req_001".to_string(),
            success: false,
            error: Some("Session not found".to_string()),
            error_code: Some("SESSION_NOT_FOUND".to_string()),
            payload: None,
        };

        #[derive(Debug, Deserialize)]
        struct TestPayload {
            #[allow(dead_code)]
            value: i32,
        }

        let extracted: Result<TestPayload, _> = response.extract();
        assert!(extracted.is_err());

        match extracted.unwrap_err() {
            ClientError::ServiceError { code, message } => {
                assert_eq!(code, "SESSION_NOT_FOUND");
                assert_eq!(message, "Session not found");
            }
            _ => panic!("Expected ServiceError"),
        }
    }

    #[test]
    fn test_correlation_id_uniqueness() {
        let id1 = generate_correlation_id();
        let id2 = generate_correlation_id();
        let id3 = generate_correlation_id();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_ipc_client_config_default() {
        let config = IpcClientConfig::default();

        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay, Duration::from_millis(100));
        assert_eq!(config.max_message_size, 1_048_576);
    }
}

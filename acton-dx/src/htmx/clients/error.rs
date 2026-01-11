//! Client error types for microservice communication.

use std::fmt;

/// Error type for service client operations.
#[derive(Debug)]
pub enum ClientError {
    /// Service is not configured.
    NotConfigured(&'static str),
    /// Connection to service failed.
    ConnectionFailed(String),
    /// Request to service failed.
    RequestFailed(String),
    /// Service returned an error status (legacy, use ServiceError with code for IPC).
    #[deprecated(note = "Use ServiceError { code, message } for new code")]
    ServiceErrorLegacy(String),
    /// Service returned an error with code and message.
    ServiceError {
        /// Machine-readable error code.
        code: String,
        /// Human-readable error message.
        message: String,
    },
    /// Response parsing failed.
    ResponseError(String),
    /// Circuit breaker is open.
    CircuitOpen(&'static str),
    /// Request timed out.
    Timeout,
    /// Serialization failed.
    SerializationError(String),
    /// Deserialization failed.
    DeserializationError(String),
    /// I/O error during communication.
    IoError(String),
    /// IPC socket not found.
    SocketNotFound(String),
    /// Transport not available.
    TransportUnavailable(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured(service) => write!(f, "Service not configured: {service}"),
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::RequestFailed(msg) => write!(f, "Request failed: {msg}"),
            #[allow(deprecated)]
            Self::ServiceErrorLegacy(msg) => write!(f, "Service error: {msg}"),
            Self::ServiceError { code, message } => {
                write!(f, "Service error [{code}]: {message}")
            }
            Self::ResponseError(msg) => write!(f, "Response error: {msg}"),
            Self::CircuitOpen(service) => write!(f, "Circuit breaker open for: {service}"),
            Self::Timeout => write!(f, "Request timed out"),
            Self::SerializationError(msg) => write!(f, "Serialization error: {msg}"),
            Self::DeserializationError(msg) => write!(f, "Deserialization error: {msg}"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::SocketNotFound(path) => write!(f, "IPC socket not found: {path}"),
            Self::TransportUnavailable(msg) => write!(f, "Transport unavailable: {msg}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<tonic::transport::Error> for ClientError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::ConnectionFailed(err.to_string())
    }
}

impl From<tonic::Status> for ClientError {
    fn from(status: tonic::Status) -> Self {
        Self::ServiceError {
            code: status.code().to_string(),
            message: status.message().to_string(),
        }
    }
}

impl From<std::io::Error> for ClientError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
    }
}

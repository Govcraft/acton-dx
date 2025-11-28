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
    /// Service returned an error status.
    ServiceError(String),
    /// Response parsing failed.
    ResponseError(String),
    /// Circuit breaker is open.
    CircuitOpen(&'static str),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured(service) => write!(f, "Service not configured: {service}"),
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::RequestFailed(msg) => write!(f, "Request failed: {msg}"),
            Self::ServiceError(msg) => write!(f, "Service error: {msg}"),
            Self::ResponseError(msg) => write!(f, "Response error: {msg}"),
            Self::CircuitOpen(service) => write!(f, "Circuit breaker open for: {service}"),
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
        Self::ServiceError(format!("{}: {}", status.code(), status.message()))
    }
}

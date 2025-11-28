//! Cedar authorization service for Acton DX.
//!
//! Provides Cedar policy-based authorization as a gRPC service.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod services;

pub use config::{CedarServiceConfig, PolicyConfig, ServiceConfig};
pub use services::CedarServiceImpl;

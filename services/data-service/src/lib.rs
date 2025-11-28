//! Data service for Acton DX.
//!
//! Provides database abstraction layer with query execution and migrations.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod services;

pub use config::{DataServiceConfig, DatabaseConfig, ServiceConfig};
pub use services::DataServiceImpl;

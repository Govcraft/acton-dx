//! Cache service for Acton DX.
//!
//! Provides Redis caching, rate limiting, and distributed session storage.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod services;

pub use config::{CacheServiceConfig, RedisConfig, ServiceConfig};
pub use services::CacheServiceImpl;

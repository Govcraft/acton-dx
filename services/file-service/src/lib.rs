//! File service for Acton DX.
//!
//! Provides file storage, uploads, downloads, and URL generation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod services;

pub use config::FileServiceConfig;
pub use services::FileServiceImpl;

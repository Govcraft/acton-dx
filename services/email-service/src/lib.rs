//! Email service for Acton DX.
//!
//! Provides email sending with SMTP backend support.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod services;

pub use config::EmailServiceConfig;
pub use services::EmailServiceImpl;
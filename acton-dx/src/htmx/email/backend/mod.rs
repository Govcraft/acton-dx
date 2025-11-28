//! Email backend implementations
//!
//! This module provides different backend implementations for sending emails:
//! - **SMTP**: Send emails via SMTP server (production)
//! - **AWS SES**: Send emails via Amazon SES (production, AWS environments)
//! - **Console**: Print emails to console (development)
//! - **Microservices**: Send via email-service gRPC endpoint (microservices mode)

pub mod aws_ses;
pub mod console;
#[cfg(feature = "microservices")]
pub mod microservices;
pub mod smtp;

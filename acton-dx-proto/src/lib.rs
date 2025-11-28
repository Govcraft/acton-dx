//! Acton DX Protocol Buffer definitions.
//!
//! This crate provides gRPC service definitions and message types for all
//! Acton DX microservices.
//!
//! # Services
//!
//! - [`auth`] - Authentication, sessions, passwords, CSRF tokens, and users
//! - [`data`] - Database queries, transactions, and migrations
//! - [`cedar`] - Cedar-based authorization
//! - [`cache`] - Redis caching, rate limiting, hash and list operations
//! - [`email`] - Email sending and validation
//! - [`file`] - File storage, uploads, and serving
//!
//! # Generated Code
//!
//! All types in this crate are auto-generated from Protocol Buffer definitions
//! using `tonic-build`. The generated code includes gRPC client and server
//! implementations for each service.
//!
//! Note: Clippy lints for generated code are configured in `Cargo.toml` since
//! we cannot modify the auto-generated protobuf code.

/// Auth service protocol definitions.
///
/// Includes session management, password hashing/verification,
/// CSRF token handling, and user CRUD operations.
pub mod auth {
    /// Version 1 of the auth service API.
    #[allow(missing_docs)]
    pub mod v1 {
        tonic::include_proto!("acton.dx.auth.v1");
    }
}

/// Data service protocol definitions.
///
/// Provides database query execution, transactions,
/// and migration management.
pub mod data {
    /// Version 1 of the data service API.
    #[allow(missing_docs)]
    pub mod v1 {
        tonic::include_proto!("acton.dx.data.v1");
    }
}

/// Cedar authorization service protocol definitions.
///
/// Enables Cedar policy-based authorization checks
/// with batch support and policy validation.
pub mod cedar {
    /// Version 1 of the cedar service API.
    #[allow(missing_docs)]
    pub mod v1 {
        tonic::include_proto!("acton.dx.cedar.v1");
    }
}

/// Cache service protocol definitions.
///
/// Redis operations including key-value storage,
/// rate limiting, hash operations, and list operations.
pub mod cache {
    /// Version 1 of the cache service API.
    #[allow(missing_docs)]
    pub mod v1 {
        tonic::include_proto!("acton.dx.cache.v1");
    }
}

/// Email service protocol definitions.
///
/// Email composition and sending with support for
/// attachments, HTML/text bodies, and batch operations.
pub mod email {
    /// Version 1 of the email service API.
    #[allow(missing_docs)]
    pub mod v1 {
        tonic::include_proto!("acton.dx.email.v1");
    }
}

/// File service protocol definitions.
///
/// File storage operations including streaming uploads/downloads,
/// metadata management, and URL generation.
pub mod file {
    /// Version 1 of the file service API.
    #[allow(missing_docs)]
    pub mod v1 {
        tonic::include_proto!("acton.dx.file.v1");
    }
}

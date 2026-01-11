//! Observability (logging, tracing, metrics)
//!
//! Provides structured logging for HTMX applications with environment-based
//! configuration. Uses pretty formatting in development and JSON in production.
//!
//! # Example
//!
//! ```rust,no_run
//! use acton_dx::observability;
//!
//! # fn main() -> anyhow::Result<()> {
//! observability::init()?;
//! tracing::info!("Application started");
//! # Ok(())
//! # }
//! ```
//!
//! For metrics collection, see the [`metrics`] submodule.

pub mod metrics;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize observability stack
///
/// Sets up:
/// - Structured logging with JSON formatting (production) or pretty formatting (dev)
/// - Environment-based log level filtering via `RUST_LOG`
///
/// # Errors
///
/// Returns an error if the tracing subscriber global default cannot be set
/// (typically because it was already initialized).
///
/// # Example
///
/// ```rust,no_run
/// use acton_dx::observability;
///
/// # fn main() -> anyhow::Result<()> {
/// observability::init()?;
/// tracing::info!("Application started");
/// # Ok(())
/// # }
/// ```
pub fn init() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            EnvFilter::new("debug,acton_dx=trace")
        } else {
            EnvFilter::new("info")
        }
    });

    #[cfg(debug_assertions)]
    {
        // Pretty formatting for development
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        // JSON formatting for production
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Note: init() can only be called once per process, so we don't test it directly.
    // The functionality is verified through integration tests.
}

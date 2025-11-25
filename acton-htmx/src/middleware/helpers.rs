//! Middleware layer construction helpers
//!
//! This module provides helper utilities for creating middleware layers with
//! consistent patterns across the framework. These helpers reduce boilerplate
//! while maintaining clarity and idiomatic Rust code.
//!
//! # HTMX Request Detection
//!
//! The [`is_htmx_request`] function provides centralized HTMX request detection
//! used by all middleware and extractors in the framework.

use axum::http::HeaderMap;

/// Check if the request is an HTMX request.
///
/// HTMX requests include the `HX-Request: true` header. This helper function
/// provides a single source of truth for HTMX detection across all extractors
/// and middleware in the framework.
///
/// # Arguments
///
/// * `headers` - The request headers to check
///
/// # Returns
///
/// `true` if the request is from HTMX, `false` otherwise.
///
/// # Example
///
/// ```rust
/// use axum::http::HeaderMap;
/// use acton_htmx::middleware::helpers::is_htmx_request;
///
/// let mut headers = HeaderMap::new();
/// assert!(!is_htmx_request(&headers));
///
/// headers.insert("HX-Request", "true".parse().unwrap());
/// assert!(is_htmx_request(&headers));
/// ```
#[must_use]
#[inline]
pub fn is_htmx_request(headers: &HeaderMap) -> bool {
    headers
        .get("HX-Request")
        .and_then(|v| v.to_str().ok())
        == Some("true")
}

/// Helper macro for creating standard middleware layer constructors
///
/// This macro generates the common constructor patterns that most middleware
/// layers need: `new()`, `with_config()`, and `from_handle()`.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::middleware::middleware_constructors;
/// use acton_htmx::state::ActonHtmxState;
/// use acton_reactive::prelude::AgentHandle;
///
/// pub struct MyMiddleware {
///     agent_handle: AgentHandle<MyAgent>,
/// }
///
/// middleware_constructors!(
///     MyMiddleware,        // Middleware type
///     MyAgent,             // Agent type
///     agent_handle,        // Field name for the handle
///     MyConfig             // Config type
/// );
/// ```
///
/// This generates:
/// ```rust,ignore
/// impl MyMiddleware {
///     pub fn new(state: &ActonHtmxState) -> Self {
///         Self {
///             agent_handle: state.my_agent().clone(),
///         }
///     }
///
///     pub fn with_config(state: &ActonHtmxState, _config: MyConfig) -> Self {
///         Self::new(state)
///     }
///
///     pub fn from_handle(handle: AgentHandle<MyAgent>) -> Self {
///         Self {
///             agent_handle: handle,
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! middleware_constructors {
    ($middleware:ty, $agent:ty, $field:ident, $config:ty, $state_method:ident) => {
        impl $middleware {
            /// Create middleware from application state
            ///
            /// This is the standard way to create middleware when adding it to your router.
            #[must_use]
            pub fn new(state: &$crate::state::ActonHtmxState) -> Self {
                Self {
                    $field: state.$state_method().clone(),
                }
            }

            /// Create middleware with custom configuration
            ///
            /// Note: Most middleware layers ignore the config parameter and use
            /// the configuration from the agent initialization. This method exists
            /// for API consistency.
            #[must_use]
            pub fn with_config(state: &$crate::state::ActonHtmxState, _config: $config) -> Self {
                Self::new(state)
            }

            /// Create middleware directly from an agent handle
            ///
            /// This is useful for testing or when you have a custom agent instance.
            #[must_use]
            pub const fn from_handle(handle: acton_reactive::prelude::AgentHandle<$agent>) -> Self {
                Self { $field: handle }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_htmx_request_with_header() {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "true".parse().unwrap());
        assert!(is_htmx_request(&headers));
    }

    #[test]
    fn test_is_htmx_request_without_header() {
        let headers = HeaderMap::new();
        assert!(!is_htmx_request(&headers));
    }

    #[test]
    fn test_is_htmx_request_with_wrong_value() {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "false".parse().unwrap());
        assert!(!is_htmx_request(&headers));
    }

    #[test]
    fn test_is_htmx_request_with_empty_value() {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "".parse().unwrap());
        assert!(!is_htmx_request(&headers));
    }

    #[test]
    fn test_is_htmx_request_case_sensitive() {
        let mut headers = HeaderMap::new();
        headers.insert("HX-Request", "True".parse().unwrap());
        assert!(!is_htmx_request(&headers));
    }

    // Macro usage is tested within the actual middleware implementations
    // (session, csrf, auth) which use this macro.
}

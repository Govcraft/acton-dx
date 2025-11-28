//! Middleware layers for acton-dx
//!
//! Provides middleware for:
//! - Session management (cookie-based sessions with agent backend)
//! - Authentication (route protection)
//! - CSRF protection (token-based CSRF validation)
//! - Security headers (automatic security header injection)
//! - File serving (range requests, caching, access control)
//! - Cedar authorization (policy-based access control, requires cedar feature)
//! - Rate limiting (Redis-backed or in-memory, per-user/IP/route limits)

pub mod auth;
#[cfg(feature = "cedar")]
pub mod cedar;
#[cfg(feature = "cedar")]
pub mod cedar_template;
pub mod csrf;
pub mod file_serving;
pub mod helpers;
pub mod rate_limit;
pub mod security_headers;
pub mod session;

// Re-exports are intentionally public even if not used within the crate itself
#[allow(unused_imports)]
pub use auth::{AuthMiddleware, AuthMiddlewareError};
#[cfg(feature = "cedar")]
#[allow(unused_imports)]
pub use cedar::{CedarAuthz, CedarAuthzBuilder, CedarError};
#[cfg(feature = "cedar")]
#[allow(unused_imports)]
pub use cedar_template::{AuthzContext, AuthzContextBuilder};
#[allow(unused_imports)]
pub use csrf::{
    CsrfConfig, CsrfLayer, CsrfMiddleware, CSRF_FORM_FIELD, CSRF_HEADER_NAME,
};
#[allow(unused_imports)]
pub use file_serving::{
    serve_file, FileAccessControl, FileServingError, FileServingMiddleware,
};
#[allow(unused_imports)]
pub use rate_limit::{RateLimit, RateLimitError};
#[allow(unused_imports)]
pub use security_headers::{
    FrameOptions, HstsConfig, ReferrerPolicy, SecurityHeadersConfig, SecurityHeadersLayer,
    SecurityHeadersMiddleware,
};
#[allow(unused_imports)]
pub use session::{SameSite, SessionConfig, SessionLayer, SessionMiddleware, SESSION_COOKIE_NAME};
#[cfg(feature = "microservices")]
#[allow(unused_imports)]
pub use session::{MicroservicesSessionLayer, MicroservicesSessionMiddleware};
#[allow(unused_imports)]
pub use helpers::is_htmx_request;

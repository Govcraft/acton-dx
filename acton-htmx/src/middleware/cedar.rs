//! Cedar authorization middleware for acton-htmx
//!
//! This middleware integrates AWS Cedar policy-based authorization into acton-htmx.
//! It validates authorization requests against Cedar policies after session authentication.
//!
//! # Key Differences from acton-service
//!
//! - **Session-based auth**: Extracts principal from User (via session), not JWT claims
//! - **HTMX responses**: Returns HTMX partials for 403 errors instead of full pages
//! - **Template integration**: Provides helpers for authorization checks in templates
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use acton_htmx::middleware::cedar::CedarAuthz;
//! use acton_htmx::config::CedarConfig;
//!
//! let cedar = CedarAuthz::from_config(cedar_config).await?;
//!
//! let app = Router::new()
//!     .route("/posts/:id", put(update_post))
//!     .layer(axum::middleware::from_fn_with_state(cedar.clone(), CedarAuthz::middleware));
//! ```

#[cfg(feature = "cedar")]
use axum::{
    body::Body,
    extract::{MatchedPath, Request, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

#[cfg(feature = "cedar")]
use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityUid, PolicySet, Request as CedarRequest,
};

#[cfg(feature = "cedar")]
use chrono::{Datelike, Timelike};

#[cfg(feature = "cedar")]
use serde_json::json;

#[cfg(feature = "cedar")]
use std::sync::Arc;

#[cfg(feature = "cedar")]
use tokio::sync::RwLock;

#[cfg(feature = "cedar")]
use crate::{auth::user::User, config::{CedarConfig, FailureMode}};

#[cfg(feature = "cedar")]
use thiserror::Error;

/// Cedar authorization errors
#[cfg(feature = "cedar")]
#[derive(Debug, Error)]
pub enum CedarError {
    /// Configuration error
    #[error("Cedar configuration error: {0}")]
    Config(String),

    /// Policy file error
    #[error("Policy file error: {0}")]
    PolicyFile(String),

    /// Policy parsing error
    #[error("Policy parsing error: {0}")]
    PolicyParsing(String),

    /// Authorization denied
    #[error("Authorization denied: {0}")]
    Forbidden(String),

    /// Unauthorized (not authenticated)
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Join error
    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

#[cfg(feature = "cedar")]
impl IntoResponse for CedarError {
    #[allow(clippy::too_many_lines)] // Inline HTML template, acceptable
    fn into_response(self) -> Response {
        use axum::http::header;

        let (status, message, redirect_to_login) = match self {
            Self::Forbidden(_) => (
                StatusCode::FORBIDDEN,
                "Access denied. You do not have permission to perform this action.",
                false,
            ),
            Self::Unauthorized(_) => (
                StatusCode::UNAUTHORIZED,
                "Authentication required. Please sign in.",
                true,
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal error occurred.",
                false,
            ),
        };

        tracing::error!(error = ?self, "Cedar authorization error");

        // For unauthorized (not authenticated), redirect to login via HX-Redirect header
        if redirect_to_login {
            return axum::response::Response::builder()
                .status(status)
                .header("HX-Redirect", "/auth/login")
                .body(Body::empty())
                .unwrap_or_else(|_| (status, message).into_response());
        }

        // For forbidden (authenticated but not authorized), return 403 with message
        // Build HTML response that works for both HTMX and full page requests
        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{status} - Access Denied</title>
    <style>
        body {{
            font-family: system-ui, -apple-system, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }}
        .error-container {{
            background: white;
            border-radius: 12px;
            box-shadow: 0 10px 40px rgba(0, 0, 0, 0.15);
            padding: 3rem;
            text-align: center;
            max-width: 500px;
            margin: 2rem;
        }}
        h1 {{
            color: #dc2626;
            font-size: 4rem;
            margin: 0 0 1rem 0;
            font-weight: 700;
        }}
        h2 {{
            color: #374151;
            font-size: 1.5rem;
            margin: 0 0 1rem 0;
            font-weight: 600;
        }}
        p {{
            color: #6b7280;
            line-height: 1.6;
            margin: 0 0 2rem 0;
        }}
        .actions {{
            display: flex;
            gap: 1rem;
            justify-content: center;
            flex-wrap: wrap;
        }}
        a {{
            display: inline-block;
            padding: 0.75rem 1.5rem;
            border-radius: 6px;
            text-decoration: none;
            font-weight: 500;
            transition: all 0.2s;
        }}
        .primary {{
            background: #667eea;
            color: white;
        }}
        .primary:hover {{
            background: #5a67d8;
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
        }}
        .secondary {{
            background: #e5e7eb;
            color: #374151;
        }}
        .secondary:hover {{
            background: #d1d5db;
        }}
    </style>
</head>
<body>
    <div class="error-container">
        <h1>{status}</h1>
        <h2>Access Denied</h2>
        <p>{message}</p>
        <div class="actions">
            <a href="javascript:history.back()" class="secondary">Go Back</a>
            <a href="/" class="primary">Return Home</a>
        </div>
    </div>
</body>
</html>"#,
            status = status.as_u16(),
            message = message
        );

        (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
    }
}

/// Builder for Cedar authorization middleware
///
/// Use this to construct a `CedarAuthz` instance with custom configuration.
///
/// # Examples
///
/// Simple case (defaults):
/// ```rust,ignore
/// let cedar = CedarAuthz::builder(cedar_config)
///     .build()
///     .await?;
/// ```
///
/// With custom path normalizer:
/// ```rust,ignore
/// let cedar = CedarAuthz::builder(cedar_config)
///     .with_path_normalizer(normalize_fn)
///     .build()
///     .await?;
/// ```
#[cfg(feature = "cedar")]
pub struct CedarAuthzBuilder {
    config: CedarConfig,
    path_normalizer: Option<fn(&str) -> String>,
}

#[cfg(feature = "cedar")]
impl CedarAuthzBuilder {
    /// Create a new builder with the given configuration
    #[must_use]
    pub fn new(config: CedarConfig) -> Self {
        Self {
            config,
            path_normalizer: None,
        }
    }

    /// Set a custom path normalizer
    ///
    /// By default, Cedar uses a generic path normalizer that replaces UUIDs and numeric IDs
    /// with `{id}` placeholders. Use this method to provide custom normalization logic for
    /// your application's specific path patterns.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn custom_normalizer(path: &str) -> String {
    ///     // Example: /articles/my-article-slug-123 -> /articles/{slug}
    ///     path.replace("/articles/", "/articles/{slug}/")
    /// }
    ///
    /// let cedar = CedarAuthz::builder(cedar_config)
    ///     .with_path_normalizer(custom_normalizer)
    ///     .build()
    ///     .await?;
    /// ```
    #[must_use]
    pub fn with_path_normalizer(mut self, normalizer: fn(&str) -> String) -> Self {
        self.path_normalizer = Some(normalizer);
        self
    }

    /// Build the CedarAuthz instance (async)
    ///
    /// This loads the Cedar policies from the configured file path.
    pub async fn build(self) -> Result<CedarAuthz, CedarError> {
        // Load policies from file (using spawn_blocking for file I/O)
        let path = self.config.policy_path.clone();
        let policies = tokio::task::spawn_blocking(move || std::fs::read_to_string(&path))
            .await??;

        let policy_set: PolicySet = policies
            .parse()
            .map_err(|e| CedarError::PolicyParsing(format!("Failed to parse Cedar policies: {e}")))?;

        Ok(CedarAuthz {
            authorizer: Arc::new(Authorizer::new()),
            policy_set: Arc::new(RwLock::new(policy_set)),
            config: Arc::new(self.config),
            path_normalizer: self.path_normalizer,
        })
    }
}

/// Cedar authorization middleware state
#[cfg(feature = "cedar")]
#[derive(Clone)]
pub struct CedarAuthz {
    /// Cedar authorizer (stateless evaluator)
    authorizer: Arc<Authorizer>,

    /// Cedar policy set (policies loaded from file)
    policy_set: Arc<RwLock<PolicySet>>,

    /// Configuration
    config: Arc<CedarConfig>,

    /// Custom path normalizer (optional, defaults to normalize_path_generic)
    path_normalizer: Option<fn(&str) -> String>,
}

#[cfg(feature = "cedar")]
impl CedarAuthz {
    /// Create a builder for CedarAuthz
    ///
    /// This is the recommended way to construct CedarAuthz instances.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cedar = CedarAuthz::builder(cedar_config)
    ///     .with_path_normalizer(normalize_fn)
    ///     .build()
    ///     .await?;
    /// ```
    #[must_use]
    pub fn builder(config: CedarConfig) -> CedarAuthzBuilder {
        CedarAuthzBuilder::new(config)
    }

    /// Create CedarAuthz from config with defaults (convenience method)
    ///
    /// This is a shortcut for `CedarAuthz::builder(config).build().await`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cedar = CedarAuthz::from_config(cedar_config).await?;
    /// ```
    pub async fn from_config(config: CedarConfig) -> Result<Self, CedarError> {
        Self::builder(config).build().await
    }

    /// Middleware function to evaluate Cedar policies
    ///
    /// This middleware:
    /// 1. Skips if Cedar is disabled
    /// 2. Skips health/ready endpoints
    /// 3. Extracts User from session (inserted by session middleware)
    /// 4. Builds Cedar principal, action, context
    /// 5. Evaluates policies
    /// 6. Returns 403 if denied, continues if allowed
    #[allow(clippy::cognitive_complexity)] // Middleware with multiple validation steps
    pub async fn middleware(
        State(authz): State<Self>,
        request: Request<Body>,
        next: Next,
    ) -> Result<Response, CedarError> {
        // Skip if Cedar is disabled
        if !authz.config.enabled {
            return Ok(next.run(request).await);
        }

        // Skip authorization for health and readiness endpoints
        let path = request.uri().path();
        if path == "/health" || path == "/ready" {
            return Ok(next.run(request).await);
        }

        // Extract User from request extensions (inserted by session middleware)
        let user = request.extensions().get::<User>().ok_or_else(|| {
            CedarError::Unauthorized(
                "Missing user session. Ensure session middleware runs before Cedar middleware."
                    .to_string(),
            )
        })?;

        // Extract request information
        let method = request.method().clone();

        // Build Cedar authorization request
        let principal = build_principal(user)?;
        let action = build_action_http(&method, &request, authz.path_normalizer)?;
        let context = build_context_http(request.headers(), user)?;

        // Build resource (generic default for now)
        let resource = build_resource()?;

        let cedar_request = CedarRequest::new(
            principal.clone(),
            action.clone(),
            resource.clone(),
            context,
            None, // Schema: None (optional)
        )
        .map_err(|e| CedarError::Internal(format!("Failed to build Cedar request: {e}")))?;

        // Evaluate policies
        let entities = build_entities(user)?;
        let response = {
            let policy_set = authz.policy_set.read().await;
            authz
                .authorizer
                .is_authorized(&cedar_request, &policy_set, &entities)
        };

        // Debug logging: Log all policy evaluations in debug mode
        if tracing::enabled!(tracing::Level::DEBUG) {
            tracing::debug!(
                principal = ?principal,
                action = ?action,
                resource = ?resource,
                decision = ?response.decision(),
                user_id = user.id,
                user_email = %user.email,
                user_roles = ?user.roles,
                user_permissions = ?user.permissions,
                diagnostics = ?response.diagnostics(),
                "Cedar policy evaluation completed"
            );
        }

        // Handle decision
        match response.decision() {
            Decision::Allow => {
                // Log successful authorization at trace level
                tracing::trace!(
                    principal = ?principal,
                    action = ?action,
                    user_id = user.id,
                    "Cedar policy allowed request"
                );

                // Allow request to proceed
                Ok(next.run(request).await)
            }
            Decision::Deny => {
                tracing::warn!(
                    principal = ?principal,
                    action = ?action,
                    user_id = user.id,
                    user_email = %user.email,
                    user_roles = ?user.roles,
                    diagnostics = ?response.diagnostics(),
                    "Cedar policy denied request"
                );

                if authz.config.failure_mode == FailureMode::Open {
                    tracing::warn!("Cedar policy denied but failure_mode=Open, allowing request");
                    Ok(next.run(request).await)
                } else {
                    Err(CedarError::Forbidden(
                        "Access denied by policy".to_string(),
                    ))
                }
            }
        }
    }

    /// Reload policies from file (for hot-reload support)
    pub async fn reload_policies(&self) -> Result<(), CedarError> {
        let path = self.config.policy_path.clone();
        let policies = tokio::task::spawn_blocking(move || std::fs::read_to_string(&path)).await??;

        let new_policy_set: PolicySet = policies
            .parse()
            .map_err(|e| CedarError::PolicyParsing(format!("Failed to parse policies: {e}")))?;

        {
            let mut policy_set = self.policy_set.write().await;
            *policy_set = new_policy_set;
        }

        tracing::info!(
            "Cedar policies reloaded from {}",
            self.config.policy_path.display()
        );
        Ok(())
    }

    /// Check if a user can perform a specific action
    ///
    /// This is a programmatic helper for authorization checks in handlers and templates.
    /// It evaluates Cedar policies for the given user, action string, and optional resource.
    ///
    /// # Arguments
    ///
    /// * `user` - The authenticated user
    /// * `action` - Action string in the format "METHOD /path" (e.g., "PUT /posts/{id}")
    /// * `resource_id` - Optional resource ID for ownership checks (future use)
    ///
    /// # Returns
    ///
    /// Returns `true` if the user is authorized, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // In a handler
    /// if cedar.can_perform(user, "DELETE /posts/{id}", None).await {
    ///     // User can delete posts
    /// }
    ///
    /// // With resource ownership check (future)
    /// if cedar.can_perform(user, "PUT /posts/{id}", Some(post_id)).await {
    ///     // User can update this specific post
    /// }
    /// ```
    #[allow(clippy::cognitive_complexity)] // Multiple Cedar request building steps
    pub async fn can_perform(
        &self,
        user: &User,
        action: &str,
        #[allow(unused_variables)] resource_id: Option<i64>,
    ) -> bool {
        // If Cedar is disabled, allow all actions
        if !self.config.enabled {
            return true;
        }

        // Build Cedar request components
        let principal = match build_principal(user) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = ?e, "Failed to build principal for can_perform");
                return false;
            }
        };

        let action_entity = match parse_action_string(action) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!(error = ?e, action = %action, "Failed to parse action for can_perform");
                return false;
            }
        };

        // Build resource (generic for now, can be extended for typed resources)
        let resource = match build_resource() {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = ?e, "Failed to build resource for can_perform");
                return false;
            }
        };

        // Build context with user attributes
        let context = match build_context_for_user(user) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = ?e, "Failed to build context for can_perform");
                return false;
            }
        };

        // Create Cedar authorization request
        let cedar_request = match CedarRequest::new(principal.clone(), action_entity.clone(), resource, context, None) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = ?e, "Failed to create Cedar request for can_perform");
                return false;
            }
        };

        // Build entities
        let entities = match build_entities(user) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = ?e, "Failed to build entities for can_perform");
                return false;
            }
        };

        // Evaluate policies
        let response = {
            let policy_set = self.policy_set.read().await;
            self.authorizer
                .is_authorized(&cedar_request, &policy_set, &entities)
        };

        // Return true if allowed, false otherwise
        matches!(response.decision(), Decision::Allow)
    }

    /// Convenience method: Check if user can update a resource
    ///
    /// Checks if the user can perform a PUT operation on the given resource path.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if cedar.can_update(user, "/posts/{id}").await {
    ///     // Show edit button
    /// }
    /// ```
    pub async fn can_update(&self, user: &User, resource_path: &str) -> bool {
        self.can_perform(user, &format!("PUT {resource_path}"), None)
            .await
    }

    /// Convenience method: Check if user can delete a resource
    ///
    /// Checks if the user can perform a DELETE operation on the given resource path.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if cedar.can_delete(user, "/posts/{id}").await {
    ///     // Show delete button
    /// }
    /// ```
    pub async fn can_delete(&self, user: &User, resource_path: &str) -> bool {
        self.can_perform(user, &format!("DELETE {resource_path}"), None)
            .await
    }

    /// Convenience method: Check if user can create a resource
    ///
    /// Checks if the user can perform a POST operation on the given resource path.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if cedar.can_create(user, "/posts").await {
    ///     // Show create button
    /// }
    /// ```
    pub async fn can_create(&self, user: &User, resource_path: &str) -> bool {
        self.can_perform(user, &format!("POST {resource_path}"), None)
            .await
    }

    /// Convenience method: Check if user can read a resource
    ///
    /// Checks if the user can perform a GET operation on the given resource path.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if cedar.can_read(user, "/posts/{id}").await {
    ///     // Show content
    /// }
    /// ```
    pub async fn can_read(&self, user: &User, resource_path: &str) -> bool {
        self.can_perform(user, &format!("GET {resource_path}"), None)
            .await
    }

    /// Get a reference to the Cedar configuration
    ///
    /// This method provides read-only access to the Cedar configuration.
    /// Useful for admin endpoints that need to display current configuration.
    #[must_use]
    pub fn config(&self) -> &CedarConfig {
        &self.config
    }
}

/// Build Cedar resource entity
///
/// Returns a generic default resource for authorization checks.
/// Most authorization policies can be implemented using just the principal (user/roles)
/// and action (HTTP method + path), without needing typed resources.
///
/// For applications that need typed resources with attributes (e.g., Post::"123"
/// with author_id for ownership checks), this can be extended in the future.
#[cfg(feature = "cedar")]
fn build_resource() -> Result<EntityUid, CedarError> {
    r#"Resource::"default""#
        .parse()
        .map_err(|e| CedarError::Internal(format!("Failed to parse resource: {e}")))
}

/// Build Cedar principal from User (session-based auth)
///
/// Extracts principal from authenticated User, not JWT claims.
/// Principal format: User::"123" (user ID)
#[cfg(feature = "cedar")]
fn build_principal(user: &User) -> Result<EntityUid, CedarError> {
    let principal_str = format!(r#"User::"{}""#, user.id);

    let principal: EntityUid = principal_str
        .parse()
        .map_err(|e| CedarError::Internal(format!("Invalid principal: {e}")))?;

    Ok(principal)
}

/// Build Cedar action from HTTP method and request
///
/// Uses Axum's MatchedPath to get the route pattern (most accurate).
/// Falls back to path normalization (custom or default) if MatchedPath is not available.
#[cfg(feature = "cedar")]
fn build_action_http(
    method: &Method,
    request: &Request<Body>,
    path_normalizer: Option<fn(&str) -> String>,
) -> Result<EntityUid, CedarError> {
    // Try to get Axum's matched path first (e.g., "/posts/:id")
    let normalized_path = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(
            || {
                // Use custom normalizer if provided, otherwise use default
                path_normalizer.map_or_else(
                    || normalize_path_generic(request.uri().path()),
                    |normalizer| normalizer(request.uri().path()),
                )
            },
            |matched| matched.as_str().to_string(),
        );

    let action_str = format!(r#"Action::"{method} {normalized_path}""#);

    let action: EntityUid = action_str
        .parse()
        .map_err(|e| CedarError::Internal(format!("Invalid action: {e}")))?;

    // Debug logging to see what action was generated
    tracing::debug!(
        method = %method,
        path = %request.uri().path(),
        normalized = %normalized_path,
        action = %action,
        "Built Cedar action"
    );

    Ok(action)
}

/// Normalize path by replacing common ID patterns with placeholders
///
/// This is a generic fallback used when Axum's MatchedPath is not available.
/// It handles the most common ID patterns:
/// - UUIDs: replaced with {id}
/// - Numeric IDs: replaced with {id}
#[cfg(feature = "cedar")]
fn normalize_path_generic(path: &str) -> String {
    // Replace UUIDs with {id}
    let uuid_pattern =
        regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
            .expect("Invalid regex");
    let path = uuid_pattern.replace_all(path, "{id}");

    // Replace numeric IDs at end of path segments
    let numeric_pattern = regex::Regex::new(r"/(\d+)(/|$)").expect("Invalid regex");
    let path = numeric_pattern.replace_all(&path, "/{id}$2");

    path.to_string()
}

/// Build Cedar context from HTTP headers and user
#[cfg(feature = "cedar")]
fn build_context_http(headers: &HeaderMap, user: &User) -> Result<Context, CedarError> {
    let mut context_map = serde_json::Map::new();

    // Add user roles
    context_map.insert("roles".to_string(), json!(user.roles));

    // Add user permissions
    context_map.insert("permissions".to_string(), json!(user.permissions));

    // Add user email
    context_map.insert("email".to_string(), json!(user.email.as_str()));

    // Add user ID
    context_map.insert("user_id".to_string(), json!(user.id));

    // Add email verification status
    context_map.insert("verified".to_string(), json!(user.email_verified));

    // Add timestamp
    let now = chrono::Utc::now();
    context_map.insert(
        "timestamp".to_string(),
        json!({
            "unix": now.timestamp(),
            "hour": now.hour(),
            "dayOfWeek": now.weekday().to_string(),
        }),
    );

    // Add IP address (from X-Forwarded-For or X-Real-IP)
    if let Some(ip) = extract_client_ip(headers) {
        context_map.insert("ip".to_string(), json!(ip));
    }

    // Add request ID if present
    if let Some(request_id) = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
    {
        context_map.insert("requestId".to_string(), json!(request_id));
    }

    // Add user-agent if present
    if let Some(user_agent) = headers.get("user-agent").and_then(|v| v.to_str().ok()) {
        context_map.insert("userAgent".to_string(), json!(user_agent));
    }

    Context::from_json_value(serde_json::Value::Object(context_map), None)
        .map_err(|e| CedarError::Internal(format!("Failed to build context: {e}")))
}

/// Extract client IP from headers
#[cfg(feature = "cedar")]
fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    // Try X-Forwarded-For header first (for proxied requests)
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(xff_str) = xff.to_str() {
            // Take first IP in comma-separated list
            return xff_str.split(',').next().map(|s| s.trim().to_string());
        }
    }

    // Try X-Real-IP header
    if let Some(xri) = headers.get("x-real-ip") {
        if let Ok(xri_str) = xri.to_str() {
            return Some(xri_str.to_string());
        }
    }

    None
}

/// Build entity hierarchy from user
///
/// Creates the principal entity (User) with roles and permissions.
#[cfg(feature = "cedar")]
fn build_entities(user: &User) -> Result<Entities, CedarError> {
    use serde_json::Value;

    // Create principal entity (User) with attributes
    let entity = json!({
        "uid": {
            "type": "User",
            "id": user.id.to_string()
        },
        "attrs": {
            "email": user.email.as_str(),
            "roles": user.roles.clone(),
            "permissions": user.permissions.clone(),
            "id": user.id,
            "verified": user.email_verified,
        },
        "parents": []
    });

    Entities::from_json_value(Value::Array(vec![entity]), None)
        .map_err(|e| CedarError::Internal(format!("Failed to build entities: {e}")))
}

/// Parse action string into Cedar EntityUid
///
/// Takes action string like "PUT /posts/{id}" and converts it to Cedar action entity.
#[cfg(feature = "cedar")]
fn parse_action_string(action: &str) -> Result<EntityUid, CedarError> {
    let action_str = format!(r#"Action::"{action}""#);
    action_str
        .parse()
        .map_err(|e| CedarError::Internal(format!("Failed to parse action '{action}': {e}")))
}

/// Build Cedar context from user alone (for programmatic checks)
///
/// Similar to `build_context_http` but without HTTP headers.
/// Used by `can_perform` and template helpers.
#[cfg(feature = "cedar")]
fn build_context_for_user(user: &User) -> Result<Context, CedarError> {
    let mut context_map = serde_json::Map::new();

    // Add user roles
    context_map.insert("roles".to_string(), json!(user.roles));

    // Add user permissions
    context_map.insert("permissions".to_string(), json!(user.permissions));

    // Add user email
    context_map.insert("email".to_string(), json!(user.email.as_str()));

    // Add user ID
    context_map.insert("user_id".to_string(), json!(user.id));

    // Add email verification status
    context_map.insert("verified".to_string(), json!(user.email_verified));

    // Add timestamp (current time for programmatic checks)
    let now = chrono::Utc::now();
    context_map.insert(
        "timestamp".to_string(),
        json!({
            "unix": now.timestamp(),
            "hour": now.hour(),
            "dayOfWeek": now.weekday().to_string(),
        }),
    );

    Context::from_json_value(serde_json::Value::Object(context_map), None)
        .map_err(|e| CedarError::Internal(format!("Failed to build context for user: {e}")))
}

#[cfg(test)]
#[cfg(feature = "cedar")]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_generic() {
        assert_eq!(
            normalize_path_generic("/api/v1/posts/123"),
            "/api/v1/posts/{id}"
        );
        assert_eq!(
            normalize_path_generic("/api/v1/posts/550e8400-e29b-41d4-a716-446655440000"),
            "/api/v1/posts/{id}"
        );
        assert_eq!(normalize_path_generic("/api/v1/posts"), "/api/v1/posts");
    }

    #[test]
    fn test_normalize_path_multiple_ids() {
        assert_eq!(
            normalize_path_generic("/api/posts/123/comments/456"),
            "/api/posts/{id}/comments/{id}"
        );
    }

    #[test]
    fn test_parse_action_string() {
        let result = parse_action_string("GET /posts");
        assert!(result.is_ok());

        let result = parse_action_string("PUT /posts/{id}");
        assert!(result.is_ok());

        let result = parse_action_string("DELETE /posts/{id}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_principal() {
        use crate::auth::user::EmailAddress;

        let user = User {
            id: 123,
            email: EmailAddress::parse("test@example.com").unwrap(),
            password_hash: "hash".to_string(),
            roles: vec!["user".to_string()],
            permissions: vec![],
            email_verified: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let principal = build_principal(&user);
        assert!(principal.is_ok());

        let principal = principal.unwrap();
        assert_eq!(principal.to_string(), r#"User::"123""#);
    }

    #[test]
    fn test_build_context_for_user() {
        use crate::auth::user::EmailAddress;

        let user = User {
            id: 123,
            email: EmailAddress::parse("test@example.com").unwrap(),
            password_hash: "hash".to_string(),
            roles: vec!["user".to_string(), "moderator".to_string()],
            permissions: vec!["read:posts".to_string()],
            email_verified: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let context = build_context_for_user(&user);
        assert!(context.is_ok());
    }

    #[test]
    fn test_build_entities() {
        use crate::auth::user::EmailAddress;

        let user = User {
            id: 123,
            email: EmailAddress::parse("test@example.com").unwrap(),
            password_hash: "hash".to_string(),
            roles: vec!["user".to_string(), "admin".to_string()],
            permissions: vec!["write:posts".to_string()],
            email_verified: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let entities = build_entities(&user);
        assert!(entities.is_ok());
    }

    #[test]
    fn test_build_resource() {
        let resource = build_resource();
        assert!(resource.is_ok());
        assert_eq!(resource.unwrap().to_string(), r#"Resource::"default""#);
    }

    // Integration tests for policy evaluation would require a full Cedar setup
    // with policy files and async runtime, so they should be in integration tests
}

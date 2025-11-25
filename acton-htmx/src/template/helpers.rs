//! Template helper functions for HTMX applications
//!
//! Provides utility functions that can be used within Askama templates
//! for common HTMX patterns.
//!
//! # HTMX Attribute Helpers
//!
//! These helpers generate proper HTMX attributes for common operations:
//!
//! ```rust
//! use acton_htmx::template::helpers::*;
//!
//! // Generate HTMX POST form attributes
//! let attrs = hx_post("/api/items", "#item-list", "innerHTML");
//! // Result: hx-post="/api/items" hx-target="#item-list" hx-swap="innerHTML"
//! ```

use crate::auth::session::FlashMessage;
use crate::template::FrameworkTemplates;
use std::sync::OnceLock;

/// Get or initialize the framework templates (lazy singleton)
fn templates() -> &'static FrameworkTemplates {
    static TEMPLATES: OnceLock<FrameworkTemplates> = OnceLock::new();
    TEMPLATES.get_or_init(|| FrameworkTemplates::new().expect("Failed to initialize templates"))
}

/// Generate CSRF token input field
///
/// **DEPRECATED**: Use `csrf_token_with(token)` instead, passing the token from your template context.
///
/// This function returns a placeholder value and should not be used in production.
/// Extract the CSRF token in your handler using the `CsrfToken` extractor and pass it
/// to your template context, then use `csrf_token_with(token)` in your template.
///
/// # Example
///
/// ```rust,ignore
/// use acton_htmx::middleware::CsrfToken;
/// use acton_htmx::template::helpers::csrf_token_with;
///
/// async fn handler(CsrfToken(token): CsrfToken) -> impl IntoResponse {
///     // Pass token to template context
///     MyTemplate { csrf_token: token }
/// }
/// ```
///
/// In your template:
/// ```html
/// {{ csrf_token_with(csrf_token) }}
/// ```
#[deprecated(since = "1.1.0", note = "Use csrf_token_with(token) instead, passing token from CsrfToken extractor")]
#[must_use]
pub fn csrf_token() -> String {
    r#"<input type="hidden" name="_csrf_token" value="placeholder">"#.to_string()
}

/// Generate CSRF token input field with a specific token value
///
/// Used when the token is provided from the session.
///
/// # Panics
///
/// Panics if the CSRF template cannot be rendered. Ensure templates are
/// initialized via `acton-htmx templates init` before using this function.
#[must_use]
pub fn csrf_token_with(token: &str) -> String {
    templates()
        .render("forms/csrf-input.html", minijinja::context! { token => token })
        .expect("Failed to render CSRF token template - run `acton-htmx templates init`")
}

/// Render flash messages as HTML
///
/// Renders a collection of flash messages with appropriate styling and ARIA attributes.
/// Each message is rendered with its level-specific CSS class and can include an optional title.
///
/// The generated HTML includes:
/// - Container div with class `flash-messages`
/// - Individual message divs with level-specific classes (`flash-success`, `flash-info`, etc.)
/// - ARIA role and live region attributes for accessibility
/// - Optional title in a `<strong>` tag
/// - Message text in a `<span>` tag
///
/// # Examples
///
/// ```rust
/// use acton_htmx::auth::session::FlashMessage;
/// use acton_htmx::template::helpers::flash_messages;
///
/// let messages = vec![
///     FlashMessage::success("Profile updated successfully"),
///     FlashMessage::error("Invalid email address"),
/// ];
///
/// let html = flash_messages(&messages);
/// assert!(html.contains("flash-success"));
/// assert!(html.contains("Profile updated"));
/// ```
///
/// In your Askama templates:
/// ```html
/// <!-- Extract flash messages in handler -->
/// {{ flash_messages(messages) }}
/// ```
///
/// # Usage with `FlashExtractor`
///
/// ```rust,ignore
/// use acton_htmx::extractors::FlashExtractor;
/// use acton_htmx::template::helpers::flash_messages;
///
/// async fn handler(FlashExtractor(messages): FlashExtractor) -> impl IntoResponse {
///     MyTemplate { flash_html: flash_messages(&messages) }
/// }
/// ```
///
/// # Panics
///
/// Panics if the flash messages template cannot be rendered. Ensure templates are
/// initialized via `acton-htmx templates init` before using this function.
#[must_use]
pub fn flash_messages(messages: &[FlashMessage]) -> String {
    if messages.is_empty() {
        return String::new();
    }

    // Convert to serializable format for template
    let msgs: Vec<_> = messages
        .iter()
        .map(|m| {
            minijinja::context! {
                css_class => m.css_class(),
                title => m.title.as_deref(),
                message => &m.message,
            }
        })
        .collect();

    templates()
        .render(
            "flash/container.html",
            minijinja::context! {
                container_class => "flash-messages",
                messages => msgs,
            },
        )
        .expect("Failed to render flash messages template - run `acton-htmx templates init`")
}

// Note: The route() helper has been removed as named routes are not currently implemented.
// Use hardcoded paths in your templates instead:
//   href="/posts/{{ post.id }}"
// If named routes are needed in the future, they can be implemented in Phase 3.

/// Generate asset URL with cache busting
///
/// **Note**: Currently returns the path as-is without cache busting.
/// Cache busting implementation is deferred to Phase 3.
///
/// # Current Behavior
///
/// Simply returns the provided path unchanged:
/// ```rust
/// use acton_htmx::template::helpers::asset;
///
/// assert_eq!(asset("/css/styles.css"), "/css/styles.css");
/// ```
///
/// # Recommended Production Approach
///
/// Until cache busting is implemented, use one of these strategies:
///
/// 1. **CDN with query string versioning**: Append a version parameter to assets
///    ```html
///    <link rel="stylesheet" href="{{ asset("/css/styles.css") }}?v=1.2.3">
///    ```
///
/// 2. **Filename-based versioning**: Include version in filename during build
///    ```html
///    <link rel="stylesheet" href="/css/styles.v1.2.3.css">
///    ```
///
/// 3. **HTTP Cache-Control headers**: Configure your static file server with proper caching headers
///    ```
///    Cache-Control: public, max-age=31536000, immutable
///    ```
///
/// # Future Implementation (Phase 3)
///
/// When implemented, this helper will:
/// - Read a manifest file (e.g., `mix-manifest.json`) generated during build
/// - Map logical paths to versioned paths (e.g., `/css/app.css` → `/css/app.abc123.css`)
/// - Support both filename hashing and query string approaches
///
/// Usage in templates: `{{ asset("/css/styles.css") }}`
#[must_use]
pub fn asset(path: &str) -> String {
    path.to_string()
}

// =============================================================================
// HTMX Attribute Helpers
// =============================================================================

/// Generate hx-post attribute with optional target and swap
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::hx_post;
///
/// let attrs = hx_post("/api/items", "#list", "innerHTML");
/// assert!(attrs.contains(r#"hx-post="/api/items""#));
/// ```
#[must_use]
pub fn hx_post(url: &str, target: &str, swap: &str) -> String {
    format!(r#"hx-post="{url}" hx-target="{target}" hx-swap="{swap}""#)
}

/// Generate hx-get attribute with optional target and swap
#[must_use]
pub fn hx_get(url: &str, target: &str, swap: &str) -> String {
    format!(r#"hx-get="{url}" hx-target="{target}" hx-swap="{swap}""#)
}

/// Generate hx-put attribute with optional target and swap
#[must_use]
pub fn hx_put(url: &str, target: &str, swap: &str) -> String {
    format!(r#"hx-put="{url}" hx-target="{target}" hx-swap="{swap}""#)
}

/// Generate hx-delete attribute with optional target and swap
#[must_use]
pub fn hx_delete(url: &str, target: &str, swap: &str) -> String {
    format!(r#"hx-delete="{url}" hx-target="{target}" hx-swap="{swap}""#)
}

/// Generate hx-patch attribute with optional target and swap
#[must_use]
pub fn hx_patch(url: &str, target: &str, swap: &str) -> String {
    format!(r#"hx-patch="{url}" hx-target="{target}" hx-swap="{swap}""#)
}

/// Generate hx-trigger attribute
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::hx_trigger;
///
/// let attr = hx_trigger("click");
/// assert_eq!(attr, r#"hx-trigger="click""#);
///
/// let attr = hx_trigger("keyup changed delay:500ms");
/// assert!(attr.contains("keyup"));
/// ```
#[must_use]
pub fn hx_trigger(trigger: &str) -> String {
    format!(r#"hx-trigger="{trigger}""#)
}

/// Generate hx-swap attribute
#[must_use]
pub fn hx_swap(strategy: &str) -> String {
    format!(r#"hx-swap="{strategy}""#)
}

/// Generate hx-target attribute
#[must_use]
pub fn hx_target(selector: &str) -> String {
    format!(r#"hx-target="{selector}""#)
}

/// Generate hx-indicator attribute for loading state
#[must_use]
pub fn hx_indicator(selector: &str) -> String {
    format!(r#"hx-indicator="{selector}""#)
}

/// Generate hx-confirm attribute for confirmation dialogs
#[must_use]
pub fn hx_confirm(message: &str) -> String {
    format!(r#"hx-confirm="{message}""#)
}

/// Generate hx-vals attribute for additional values
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::hx_vals;
///
/// let attr = hx_vals(r#"{"key": "value"}"#);
/// assert!(attr.contains("hx-vals"));
/// ```
#[must_use]
pub fn hx_vals(json: &str) -> String {
    format!(r"hx-vals='{json}'")
}

/// Generate hx-headers attribute for additional headers
#[must_use]
pub fn hx_headers(json: &str) -> String {
    format!(r"hx-headers='{json}'")
}

/// Generate hx-push-url attribute
#[must_use]
pub fn hx_push_url(url: &str) -> String {
    format!(r#"hx-push-url="{url}""#)
}

/// Generate hx-select attribute for partial selection
#[must_use]
pub fn hx_select(selector: &str) -> String {
    format!(r#"hx-select="{selector}""#)
}

/// Generate hx-select-oob attribute for out-of-band selection
#[must_use]
pub fn hx_select_oob(selector: &str) -> String {
    format!(r#"hx-select-oob="{selector}""#)
}

/// Generate hx-boost="true" for progressively enhanced links
#[must_use]
pub const fn hx_boost() -> &'static str {
    r#"hx-boost="true""#
}

/// Generate hx-disabled-elt attribute for disabling elements during request
#[must_use]
pub fn hx_disabled_elt(selector: &str) -> String {
    format!(r#"hx-disabled-elt="{selector}""#)
}

// =============================================================================
// HTML Safe Output
// =============================================================================

/// HTML-safe string wrapper
///
/// Marks a string as safe for direct HTML output (already escaped).
/// Use this when you have pre-escaped HTML that shouldn't be double-escaped.
#[derive(Debug, Clone)]
pub struct SafeString(pub String);

impl SafeString {
    /// Create a new `SafeString`
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for SafeString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SafeString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SafeString {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// =============================================================================
// HTML Escaping Utilities
// =============================================================================

/// Escape a string for safe use in HTML content
///
/// Escapes special HTML characters to prevent XSS attacks.
/// This is used internally by helpers that generate HTML.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::escape_html;
///
/// assert_eq!(escape_html("<script>alert('xss')</script>"),
///            "&lt;script&gt;alert('xss')&lt;/script&gt;");
/// assert_eq!(escape_html("Hello & goodbye"), "Hello &amp; goodbye");
/// ```
#[must_use]
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// =============================================================================
// Validation Error Helpers
// =============================================================================

/// Render validation errors for a specific field
///
/// Returns HTML markup for displaying validation errors next to form fields.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::validation_errors_for;
/// use validator::ValidationErrors;
///
/// let errors = ValidationErrors::new();
/// let html = validation_errors_for(&errors, "email");
/// ```
///
/// # Panics
///
/// Panics if the field errors template cannot be rendered. Ensure templates are
/// initialized via `acton-htmx templates init` before using this function.
#[must_use]
pub fn validation_errors_for(errors: &validator::ValidationErrors, field: &str) -> String {
    errors.field_errors().get(field).map_or_else(String::new, |field_errors| {
        let error_messages: Vec<String> = field_errors
            .iter()
            .map(|error| {
                error.message.as_ref().map_or_else(
                    || format!("{field}: {}", error.code),
                    ToString::to_string,
                )
            })
            .collect();

        templates()
            .render(
                "validation/field-errors.html",
                minijinja::context! {
                    container_class => "field-errors",
                    error_class => "error",
                    errors => error_messages,
                },
            )
            .expect("Failed to render field errors template - run `acton-htmx templates init`")
    })
}

/// Check if a field has validation errors
///
/// Useful for conditionally applying error classes in templates.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::has_error;
/// use validator::ValidationErrors;
///
/// let errors = ValidationErrors::new();
/// let has_err = has_error(&errors, "email");
/// assert!(!has_err);
/// ```
#[must_use]
pub fn has_error(errors: &validator::ValidationErrors, field: &str) -> bool {
    errors.field_errors().contains_key(field)
}

/// Get error class string if field has errors
///
/// Returns " error" or " is-invalid" if the field has errors, empty string otherwise.
/// Useful for conditionally applying CSS classes.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::error_class;
/// use validator::ValidationErrors;
///
/// let mut errors = ValidationErrors::new();
/// errors.add("email", validator::ValidationError::new("email"));
///
/// let class = error_class(&errors, "email");
/// assert_eq!(class, " error");
/// ```
#[must_use]
pub fn error_class(errors: &validator::ValidationErrors, field: &str) -> &'static str {
    if has_error(errors, field) {
        " error"
    } else {
        ""
    }
}

/// Render all validation errors as an unordered list
///
/// Useful for displaying all errors at the top of a form.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::helpers::validation_errors_list;
/// use validator::ValidationErrors;
///
/// let errors = ValidationErrors::new();
/// let html = validation_errors_list(&errors);
/// ```
///
/// # Panics
///
/// Panics if the validation summary template cannot be rendered. Ensure templates are
/// initialized via `acton-htmx templates init` before using this function.
#[must_use]
pub fn validation_errors_list(errors: &validator::ValidationErrors) -> String {
    if errors.is_empty() {
        return String::new();
    }

    let error_messages: Vec<String> = errors
        .field_errors()
        .iter()
        .flat_map(|(field, field_errors)| {
            field_errors.iter().map(move |error| {
                error.message.as_ref().map_or_else(
                    || format!("{field}: {}", error.code),
                    ToString::to_string,
                )
            })
        })
        .collect();

    templates()
        .render(
            "validation/validation-summary.html",
            minijinja::context! {
                container_class => "validation-errors",
                errors => error_messages,
            },
        )
        .expect("Failed to render validation summary template - run `acton-htmx templates init`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn test_csrf_token() {
        let token = csrf_token();
        assert!(token.contains("_csrf_token"));
        assert!(token.contains("hidden"));
    }

    #[test]
    fn test_csrf_token_with_value() {
        let token = csrf_token_with("abc123");
        assert!(token.contains(r#"value="abc123""#));
    }

    #[test]
    fn test_asset() {
        let path = asset("/css/styles.css");
        assert_eq!(path, "/css/styles.css");
    }

    #[test]
    fn test_hx_post() {
        let attrs = hx_post("/api/items", "#list", "innerHTML");
        assert!(attrs.contains("hx-post=\"/api/items\""));
        assert!(attrs.contains("hx-target=\"#list\""));
        assert!(attrs.contains("hx-swap=\"innerHTML\""));
    }

    #[test]
    fn test_hx_get() {
        let attrs = hx_get("/search", "#results", "outerHTML");
        assert!(attrs.contains(r#"hx-get="/search""#));
    }

    #[test]
    fn test_hx_trigger() {
        let attr = hx_trigger("click");
        assert_eq!(attr, r#"hx-trigger="click""#);
    }

    #[test]
    fn test_hx_confirm() {
        let attr = hx_confirm("Are you sure?");
        assert!(attr.contains("Are you sure?"));
    }

    #[test]
    fn test_hx_boost() {
        assert_eq!(hx_boost(), r#"hx-boost="true""#);
    }

    #[test]
    fn test_safe_string() {
        let safe = SafeString::new("<p>Hello</p>");
        assert_eq!(format!("{safe}"), "<p>Hello</p>");
    }

    #[test]
    fn test_safe_string_from() {
        let safe: SafeString = "test".into();
        assert_eq!(safe.0, "test");
    }

    #[test]
    fn test_validation_errors_for() {
        let mut errors = validator::ValidationErrors::new();
        errors.add(
            "email",
            validator::ValidationError::new("email")
                .with_message(std::borrow::Cow::Borrowed("Invalid email")),
        );

        let html = validation_errors_for(&errors, "email");
        assert!(html.contains("Invalid email"));
        assert!(html.contains("field-errors"));
    }

    #[test]
    fn test_validation_errors_for_no_errors() {
        let errors = validator::ValidationErrors::new();
        let html = validation_errors_for(&errors, "email");
        assert!(html.is_empty());
    }

    #[test]
    fn test_has_error() {
        let mut errors = validator::ValidationErrors::new();
        errors.add("email", validator::ValidationError::new("email"));

        assert!(has_error(&errors, "email"));
        assert!(!has_error(&errors, "password"));
    }

    #[test]
    fn test_error_class() {
        let mut errors = validator::ValidationErrors::new();
        errors.add("email", validator::ValidationError::new("email"));

        assert_eq!(error_class(&errors, "email"), " error");
        assert_eq!(error_class(&errors, "password"), "");
    }

    #[test]
    fn test_validation_errors_list() {
        let mut errors = validator::ValidationErrors::new();
        errors.add(
            "email",
            validator::ValidationError::new("email")
                .with_message(std::borrow::Cow::Borrowed("Invalid email")),
        );
        errors.add(
            "password",
            validator::ValidationError::new("length")
                .with_message(std::borrow::Cow::Borrowed("Too short")),
        );

        let html = validation_errors_list(&errors);
        assert!(html.contains("Invalid email"));
        assert!(html.contains("Too short"));
        assert!(html.contains("<ul>"));
    }

    #[test]
    fn test_validation_errors_list_empty() {
        let errors = validator::ValidationErrors::new();
        let html = validation_errors_list(&errors);
        assert!(html.is_empty());
    }

    #[test]
    fn test_flash_messages_empty() {
        let messages: Vec<FlashMessage> = vec![];
        let html = flash_messages(&messages);
        assert!(html.is_empty());
    }

    #[test]
    fn test_flash_messages_single() {
        use crate::auth::session::FlashMessage;

        let messages = vec![FlashMessage::success("Operation successful")];
        let html = flash_messages(&messages);

        assert!(html.contains("flash-messages"));
        assert!(html.contains("flash-success"));
        assert!(html.contains("Operation successful"));
        assert!(html.contains("role=\"alert\""));
        assert!(html.contains("role=\"status\""));
    }

    #[test]
    fn test_flash_messages_multiple_levels() {
        use crate::auth::session::FlashMessage;

        let messages = vec![
            FlashMessage::success("Success message"),
            FlashMessage::error("Error message"),
            FlashMessage::warning("Warning message"),
            FlashMessage::info("Info message"),
        ];
        let html = flash_messages(&messages);

        assert!(html.contains("flash-success"));
        assert!(html.contains("flash-error"));
        assert!(html.contains("flash-warning"));
        assert!(html.contains("flash-info"));
        assert!(html.contains("Success message"));
        assert!(html.contains("Error message"));
        assert!(html.contains("Warning message"));
        assert!(html.contains("Info message"));
    }

    #[test]
    fn test_flash_messages_with_title() {
        use crate::auth::session::FlashMessage;

        let messages = vec![
            FlashMessage::success("Message text").with_title("Success!"),
        ];
        let html = flash_messages(&messages);

        assert!(html.contains("<strong>Success!</strong>"));
        assert!(html.contains("Message text"));
    }

    #[test]
    fn test_flash_messages_xss_protection() {
        use crate::auth::session::FlashMessage;

        let messages = vec![
            FlashMessage::error("<script>alert('xss')</script>"),
        ];
        let html = flash_messages(&messages);

        // Should be escaped
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("Hello, world!"), "Hello, world!");
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("A & B"), "A &amp; B");
        assert_eq!(escape_html("<div>content</div>"), "&lt;div&gt;content&lt;/div&gt;");
        assert_eq!(
            escape_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert('xss')&lt;/script&gt;"
        );
    }

    #[test]
    fn test_escape_html_preserves_safe_chars() {
        assert_eq!(escape_html("Hello 123 !@#$%^*()_+-=[]{}|;:',./? "),
                   "Hello 123 !@#$%^*()_+-=[]{}|;:',./? ");
    }
}

//! Template helper functions for HTMX applications
//!
//! Provides utility functions that can be used within Askama templates
//! for common HTMX patterns.

#![allow(dead_code)]

use std::collections::HashMap;

/// Generate CSRF token input field
///
/// Returns an HTML hidden input with the CSRF token.
/// Usage in templates: `{{ csrf_token() }}`
pub fn csrf_token() -> String {
    // TODO: Integrate with actual CSRF middleware
    r#"<input type="hidden" name="_csrf_token" value="placeholder">"#.to_string()
}

/// Generate flash message HTML
///
/// Renders flash messages with appropriate styling.
/// Usage in templates: `{{ flash_messages() }}`
pub fn flash_messages() -> String {
    // TODO: Integrate with actual flash message system
    String::new()
}

/// Generate route URL
///
/// Builds a URL for a named route with parameters.
/// Usage in templates: `{{ route("posts.show", {"id": post.id}) }}`
pub fn route(_name: &str, _params: HashMap<String, String>) -> String {
    // TODO: Implement route generation
    "/".to_string()
}

/// Generate asset URL with cache busting
///
/// Returns a versioned asset URL for cache busting in production.
/// Usage in templates: `{{ asset("/css/styles.css") }}`
pub fn asset(path: &str) -> String {
    // TODO: Add cache busting in production
    path.to_string()
}

/// HTML-safe string wrapper
///
/// Marks a string as safe for direct HTML output (already escaped).
pub struct SafeString(pub String);

impl std::fmt::Display for SafeString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csrf_token() {
        let token = csrf_token();
        assert!(token.contains("_csrf_token"));
        assert!(token.contains("hidden"));
    }

    #[test]
    fn test_asset() {
        let path = asset("/css/styles.css");
        assert_eq!(path, "/css/styles.css");
    }
}

//! Out-of-band swap support for HTMX
//!
//! Provides [`HxSwapOob`] for updating multiple page elements in a single response.
//! This is useful for patterns like:
//! - Updating a counter after adding an item
//! - Showing flash messages after form submission
//! - Refreshing related content across the page

use axum::{
    http::header::CONTENT_TYPE,
    response::{Html, IntoResponse, Response},
};

/// HTMX swap strategy for out-of-band updates
///
/// Determines how the new content replaces the existing element.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SwapStrategy {
    /// Replace inner HTML of target element (default)
    #[default]
    InnerHTML,
    /// Replace entire target element
    OuterHTML,
    /// Insert content at the end of target element
    BeforeEnd,
    /// Insert content at the beginning of target element
    AfterBegin,
    /// Insert content before target element
    BeforeBegin,
    /// Insert content after target element
    AfterEnd,
    /// Delete the target element
    Delete,
    /// Do not swap (just trigger events)
    None,
}

impl SwapStrategy {
    /// Returns the HTMX swap strategy value
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InnerHTML => "innerHTML",
            Self::OuterHTML => "outerHTML",
            Self::BeforeEnd => "beforeend",
            Self::AfterBegin => "afterbegin",
            Self::BeforeBegin => "beforebegin",
            Self::AfterEnd => "afterend",
            Self::Delete => "delete",
            Self::None => "none",
        }
    }

    /// Returns the hx-swap-oob attribute value
    #[must_use]
    pub const fn oob_value(&self) -> &'static str {
        match self {
            Self::InnerHTML => "true", // "true" defaults to innerHTML
            _ => self.as_str(),
        }
    }
}

/// Out-of-band swap container
///
/// Collects multiple OOB swap targets into a single HTML response.
/// When returned from a handler, HTMX will update each target element
/// independently.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::htmx::{HxSwapOob, SwapStrategy};
///
/// let mut oob = HxSwapOob::new();
///
/// // Add multiple targets
/// oob.add("counter", "<span>42</span>", SwapStrategy::InnerHTML);
/// oob.add("messages", r#"<div class="flash">Saved!</div>"#, SwapStrategy::BeforeEnd);
///
/// // Can also chain
/// let oob = HxSwapOob::new()
///     .with("header-title", "<h1>Updated</h1>", SwapStrategy::InnerHTML)
///     .with("sidebar", "<nav>New nav</nav>", SwapStrategy::OuterHTML);
/// ```
#[derive(Debug, Default, Clone)]
pub struct HxSwapOob {
    targets: Vec<OobTarget>,
    /// Primary content that's not part of OOB swap
    primary_content: Option<String>,
}

#[derive(Debug, Clone)]
struct OobTarget {
    id: String,
    content: String,
    strategy: SwapStrategy,
}

impl HxSwapOob {
    /// Create a new empty OOB swap container
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with primary content that will be rendered first
    ///
    /// The primary content is the main response body that will be swapped
    /// into the original target. OOB elements are appended after it.
    #[must_use]
    pub fn with_primary(content: impl Into<String>) -> Self {
        Self {
            targets: Vec::new(),
            primary_content: Some(content.into()),
        }
    }

    /// Set the primary content
    pub fn set_primary(&mut self, content: impl Into<String>) -> &mut Self {
        self.primary_content = Some(content.into());
        self
    }

    /// Add an out-of-band target
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the target element (without #)
    /// * `content` - The HTML content to swap
    /// * `strategy` - How to perform the swap
    pub fn add(&mut self, id: impl Into<String>, content: impl Into<String>, strategy: SwapStrategy) -> &mut Self {
        self.targets.push(OobTarget {
            id: id.into(),
            content: content.into(),
            strategy,
        });
        self
    }

    /// Add an out-of-band target (builder pattern)
    #[must_use]
    pub fn with(mut self, id: impl Into<String>, content: impl Into<String>, strategy: SwapStrategy) -> Self {
        self.add(id, content, strategy);
        self
    }

    /// Add innerHTML swap (convenience method)
    pub fn inner_html(&mut self, id: impl Into<String>, content: impl Into<String>) -> &mut Self {
        self.add(id, content, SwapStrategy::InnerHTML)
    }

    /// Add outerHTML swap (convenience method)
    pub fn outer_html(&mut self, id: impl Into<String>, content: impl Into<String>) -> &mut Self {
        self.add(id, content, SwapStrategy::OuterHTML)
    }

    /// Add beforeend swap (append content)
    pub fn append(&mut self, id: impl Into<String>, content: impl Into<String>) -> &mut Self {
        self.add(id, content, SwapStrategy::BeforeEnd)
    }

    /// Add afterbegin swap (prepend content)
    pub fn prepend(&mut self, id: impl Into<String>, content: impl Into<String>) -> &mut Self {
        self.add(id, content, SwapStrategy::AfterBegin)
    }

    /// Check if there are any OOB targets
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty() && self.primary_content.is_none()
    }

    /// Get the number of OOB targets
    #[must_use]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Render to HTML string
    #[must_use]
    pub fn render(&self) -> String {
        let mut html = String::new();

        // Primary content first
        if let Some(ref primary) = self.primary_content {
            html.push_str(primary);
        }

        // OOB targets
        for target in &self.targets {
            html.push_str(&format!(
                r#"<div id="{}" hx-swap-oob="{}">{}</div>"#,
                target.id,
                target.strategy.oob_value(),
                target.content
            ));
        }

        html
    }
}

impl IntoResponse for HxSwapOob {
    fn into_response(self) -> Response {
        let html = self.render();
        (
            [(CONTENT_TYPE, "text/html; charset=utf-8")],
            Html(html),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_strategy_as_str() {
        assert_eq!(SwapStrategy::InnerHTML.as_str(), "innerHTML");
        assert_eq!(SwapStrategy::OuterHTML.as_str(), "outerHTML");
        assert_eq!(SwapStrategy::BeforeEnd.as_str(), "beforeend");
        assert_eq!(SwapStrategy::Delete.as_str(), "delete");
    }

    #[test]
    fn test_swap_strategy_oob_value() {
        assert_eq!(SwapStrategy::InnerHTML.oob_value(), "true");
        assert_eq!(SwapStrategy::OuterHTML.oob_value(), "outerHTML");
    }

    #[test]
    fn test_new_empty() {
        let oob = HxSwapOob::new();
        assert!(oob.is_empty());
        assert_eq!(oob.len(), 0);
    }

    #[test]
    fn test_add_target() {
        let mut oob = HxSwapOob::new();
        oob.add("test-id", "<p>Content</p>", SwapStrategy::InnerHTML);

        assert!(!oob.is_empty());
        assert_eq!(oob.len(), 1);
    }

    #[test]
    fn test_builder_pattern() {
        let oob = HxSwapOob::new()
            .with("id1", "content1", SwapStrategy::InnerHTML)
            .with("id2", "content2", SwapStrategy::OuterHTML);

        assert_eq!(oob.len(), 2);
    }

    #[test]
    fn test_render_single() {
        let mut oob = HxSwapOob::new();
        oob.add("my-id", "<span>Test</span>", SwapStrategy::InnerHTML);

        let html = oob.render();
        assert!(html.contains(r#"id="my-id""#));
        assert!(html.contains(r#"hx-swap-oob="true""#));
        assert!(html.contains("<span>Test</span>"));
    }

    #[test]
    fn test_render_multiple() {
        let oob = HxSwapOob::new()
            .with("first", "<p>First</p>", SwapStrategy::InnerHTML)
            .with("second", "<p>Second</p>", SwapStrategy::OuterHTML);

        let html = oob.render();
        assert!(html.contains(r#"id="first""#));
        assert!(html.contains(r#"id="second""#));
        assert!(html.contains(r#"hx-swap-oob="true""#));
        assert!(html.contains(r#"hx-swap-oob="outerHTML""#));
    }

    #[test]
    fn test_render_with_primary() {
        let oob = HxSwapOob::with_primary("<main>Primary</main>")
            .with("sidebar", "<nav>Nav</nav>", SwapStrategy::InnerHTML);

        let html = oob.render();
        assert!(html.starts_with("<main>Primary</main>"));
        assert!(html.contains(r#"id="sidebar""#));
    }

    #[test]
    fn test_convenience_methods() {
        let mut oob = HxSwapOob::new();
        oob.inner_html("a", "content");
        oob.outer_html("b", "content");
        oob.append("c", "content");
        oob.prepend("d", "content");

        assert_eq!(oob.len(), 4);
    }

    #[test]
    fn test_into_response() {
        let oob = HxSwapOob::new()
            .with("test", "<p>Test</p>", SwapStrategy::InnerHTML);

        let response = oob.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}

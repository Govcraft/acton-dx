//! Partial content extraction from rendered templates
//!
//! This module provides utilities to extract specific content blocks from
//! rendered HTML, enabling efficient HTMX partial updates.

use std::borrow::Cow;

/// Extract content between HTML comment markers
///
/// Looks for `<!-- HTMX_PARTIAL_START -->` and `<!-- HTMX_PARTIAL_END -->`
/// markers in the rendered HTML and returns only the content between them.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::extractor::extract_partial;
///
/// let html = r#"
/// <html>
/// <!-- HTMX_PARTIAL_START -->
/// <div>Main content</div>
/// <!-- HTMX_PARTIAL_END -->
/// </html>
/// "#;
///
/// let partial = extract_partial(html);
/// assert!(partial.contains("<div>Main content</div>"));
/// ```
#[must_use]
pub fn extract_partial(html: &str) -> Cow<'_, str> {
    const START_MARKER: &str = "<!-- HTMX_PARTIAL_START -->";
    const END_MARKER: &str = "<!-- HTMX_PARTIAL_END -->";

    if let Some(start_pos) = html.find(START_MARKER) {
        let content_start = start_pos + START_MARKER.len();
        if let Some(end_pos) = html[content_start..].find(END_MARKER) {
            let content = &html[content_start..content_start + end_pos];
            return Cow::Borrowed(content.trim());
        }
    }

    // If no markers found, return the full HTML
    Cow::Borrowed(html)
}

/// Extract content by CSS selector-like ID
///
/// Looks for content within `<div id="target_id">...</div>` tags.
/// This is a simple implementation that works with well-formed HTML.
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::extractor::extract_by_id;
///
/// let html = r#"
/// <html>
/// <body>
///     <div id="header">Header</div>
///     <div id="main-content">
///         <p>Main content here</p>
///     </div>
///     <div id="footer">Footer</div>
/// </body>
/// </html>
/// "#;
///
/// let main = extract_by_id(html, "main-content");
/// assert!(main.is_some());
/// assert!(main.unwrap().contains("<p>Main content here</p>"));
/// ```
#[must_use]
pub fn extract_by_id<'a>(html: &'a str, id: &str) -> Option<Cow<'a, str>> {
    // Match <div id="target"> or <div id='target'> with optional spaces
    let pattern_double = format!(r#"<div id="{id}""#);
    let pattern_single = format!(r"<div id='{id}'");

    // Find the start tag
    let start_pos = if let Some(pos) = html.find(&pattern_double) {
        pos
    } else if let Some(pos) = html.find(&pattern_single) {
        pos
    } else {
        return None;
    };

    // Find the end of the opening tag (>)
    let tag_start = &html[start_pos..];
    let tag_end_pos = tag_start.find('>')?;
    let content_start = start_pos + tag_end_pos + 1;
    let remaining = &html[content_start..];

    // Find matching closing tag by counting div depth
    let mut depth = 1;
    let mut pos = 0;

    while pos < remaining.len() && depth > 0 {
        if remaining[pos..].starts_with("<div") {
            // Check if this is a real opening tag
            let next_char_pos = pos + 4;
            if next_char_pos < remaining.len() {
                let next_char = remaining.chars().nth(next_char_pos);
                if next_char == Some('>') || next_char == Some(' ') {
                    depth += 1;
                }
            }
            pos += 1;
        } else if remaining[pos..].starts_with("</div>") {
            depth -= 1;
            if depth == 0 {
                return Some(Cow::Borrowed(&remaining[..pos]));
            }
            pos += 6;
        } else {
            pos += 1;
        }
    }

    None
}

/// Extract main content block from template
///
/// Uses multiple strategies to find the main content:
/// 1. Look for HTMX partial markers
/// 2. Look for `#main-content` div
/// 3. Look for `#content` div
/// 4. Return full HTML if none found
///
/// # Examples
///
/// ```rust
/// use acton_htmx::template::extractor::extract_main_content;
///
/// let html = r#"
/// <html>
/// <body>
///     <nav>Navigation</nav>
///     <div id="main-content">
///         <h1>Welcome</h1>
///         <p>Main page content</p>
///     </div>
///     <footer>Footer</footer>
/// </body>
/// </html>
/// "#;
///
/// let content = extract_main_content(html);
/// assert!(content.contains("<h1>Welcome</h1>"));
/// assert!(!content.contains("<nav>"));
/// ```
#[must_use]
pub fn extract_main_content(html: &str) -> Cow<'_, str> {
    // Strategy 1: Check for explicit HTMX markers
    let partial = extract_partial(html);
    if partial.as_ref() != html {
        return partial;
    }

    // Strategy 2: Look for #main-content
    if let Some(content) = extract_by_id(html, "main-content") {
        return content;
    }

    // Strategy 3: Look for #content
    if let Some(content) = extract_by_id(html, "content") {
        return content;
    }

    // Strategy 4: Return full HTML
    Cow::Borrowed(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_partial_with_markers() {
        let html = r#"
<html>
<head><title>Test</title></head>
<body>
<!-- HTMX_PARTIAL_START -->
<div id="content">Main content here</div>
<!-- HTMX_PARTIAL_END -->
<footer>Footer</footer>
</body>
</html>
"#;

        let partial = extract_partial(html);
        assert!(partial.contains("<div id=\"content\">Main content here</div>"));
        assert!(!partial.contains("<footer>"));
    }

    #[test]
    fn test_extract_partial_no_markers() {
        let html = "<div>No markers</div>";
        let partial = extract_partial(html);
        assert_eq!(partial, html);
    }

    #[test]
    fn test_extract_by_id_simple() {
        let html = r#"<div id="test"><p>Content</p></div>"#;
        let content = extract_by_id(html, "test");
        assert!(content.is_some());
        assert_eq!(content.unwrap(), "<p>Content</p>");
    }

    #[test]
    fn test_extract_by_id_nested() {
        let html = r#"
<div id="outer">
    <div id="inner">
        <p>Nested content</p>
    </div>
</div>
"#;
        let content = extract_by_id(html, "outer");
        assert!(content.is_some());
        assert!(content.unwrap().contains("<div id=\"inner\">"));
    }

    #[test]
    fn test_extract_by_id_not_found() {
        let html = r#"<div id="other">Content</div>"#;
        let content = extract_by_id(html, "test");
        assert!(content.is_none());
    }

    #[test]
    fn test_extract_main_content_with_markers() {
        let html = r"
<html>
<!-- HTMX_PARTIAL_START -->
<div>Partial</div>
<!-- HTMX_PARTIAL_END -->
</html>
";
        let content = extract_main_content(html);
        assert!(content.contains("<div>Partial</div>"));
        assert!(!content.contains("<html>"));
    }

    #[test]
    fn test_extract_main_content_by_id() {
        let html = r#"
<html>
<body>
    <nav>Nav</nav>
    <div id="main-content"><p>Main</p></div>
    <footer>Footer</footer>
</body>
</html>
"#;
        let content = extract_main_content(html);
        assert!(content.contains("<p>Main</p>"));
        assert!(!content.contains("<nav>"));
    }

    #[test]
    fn test_extract_main_content_fallback() {
        let html = "<div>Full HTML</div>";
        let content = extract_main_content(html);
        assert_eq!(content, html);
    }
}

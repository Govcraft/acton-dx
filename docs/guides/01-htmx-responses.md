# HTMX Response Guide

This guide covers all HTMX response types available in acton-htmx, including both `axum-htmx` types and acton-htmx extensions.

## Overview

HTMX works by sending HTTP headers that tell the client how to update the page. acton-htmx provides type-safe wrappers for these headers.

## Response Types

### 1. HxRedirect - Client-Side Redirect

Redirect the browser to a new URL:

```rust
use acton_htmx::prelude::*;

async fn save_post() -> HxRedirect {
    // Save post logic...

    HxRedirect(axum::http::Uri::from_static("/posts"))
}
```

**HTMX Header**: `HX-Redirect: /posts`

**Use when**: You want to redirect after a successful form submission.

### 2. HxRefresh - Reload Current Page

Tell the browser to refresh the current page:

```rust
async fn delete_post() -> HxRefresh {
    // Delete post logic...

    HxRefresh
}
```

**HTMX Header**: `HX-Refresh: true`

**Use when**: The current page needs to reload to reflect changes.

### 3. HxTrigger - Client-Side Events

Trigger custom JavaScript events on the client:

```rust
use axum_htmx::HxResponseTrigger;
use serde_json::json;

async fn create_post() -> impl axum::response::IntoResponse {
    // Create post logic...

    HxResponseTrigger::normal(["postCreated"])
}

// With event data
async fn update_post(id: i64) -> impl axum::response::IntoResponse {
    // Update post logic...

    HxResponseTrigger::normal([("postUpdated", json!({"id": id}))])
}
```

**HTMX Header**: `HX-Trigger: postCreated` or `HX-Trigger: {"postUpdated": {"id": 42}}`

**Use when**: You need to notify other parts of the page about changes.

**Trigger Timing**:
- `HxResponseTrigger::normal()` - Trigger immediately
- `HxResponseTrigger::after_settle()` - Trigger after DOM settles
- `HxResponseTrigger::after_swap()` - Trigger after content swap

### 4. HxReswap - Change Swap Strategy

Override the swap strategy for this response:

```rust
use axum_htmx::{HxReswap, SwapOption};

async fn update_nav() -> (HxReswap, Html<&'static str>) {
    let swap = HxReswap(vec![SwapOption::OuterHTML]);
    let html = Html("<nav>New navigation</nav>");

    (swap, html)
}
```

**HTMX Header**: `HX-Reswap: outerHTML`

**Swap options**:
- `InnerHTML` - Replace inner HTML (default)
- `OuterHTML` - Replace entire element
- `BeforeBegin` - Insert before element
- `AfterBegin` - Insert at start of element
- `BeforeEnd` - Insert at end of element
- `AfterEnd` - Insert after element
- `Delete` - Delete element
- `None` - Don't swap

**Use when**: You need fine-grained control over how content is inserted.

### 5. HxRetarget - Change Target Element

Override the target element for this response:

```rust
use axum_htmx::HxRetarget;

async fn show_error() -> (HxRetarget, Html<&'static str>) {
    let target = HxRetarget("#error-container".to_string());
    let html = Html(r#"<div class="error">Something went wrong</div>"#);

    (target, html)
}
```

**HTMX Header**: `HX-Retarget: #error-container`

**Use when**: You want to update a different element than specified in the request.

### 6. HxReselect - Select Content from Response

Select a specific part of the response to swap:

```rust
use axum_htmx::HxReselect;

async fn get_page() -> (HxReselect, Html<&'static str>) {
    let reselect = HxReselect("#main-content".to_string());
    let html = Html(r#"
        <html>
            <head><title>Page</title></head>
            <body>
                <div id="main-content">This will be swapped</div>
                <footer>This will be ignored</footer>
            </body>
        </html>
    "#);

    (reselect, html)
}
```

**HTMX Header**: `HX-Reselect: #main-content`

**Use when**: You want to return a full page but only swap part of it.

### 7. HxPushUrl - Update Browser URL

Push a new URL to the browser history:

```rust
use axum_htmx::HxPushUrl;

async fn load_post(Path(id): Path<i64>) -> impl axum::response::IntoResponse {
    let post = load_post_from_db(id).await;

    (
        HxPushUrl(format!("/posts/{id}").parse().unwrap()),
        post.render_html()
    )
}
```

**HTMX Header**: `HX-Push-Url: /posts/42`

**Use when**: You want the URL bar to reflect the current view.

### 8. HxReplaceUrl - Replace Browser URL

Replace the current URL without adding to history:

```rust
use axum_htmx::HxReplaceUrl;

async fn filter_posts(Query(params): Query<FilterParams>) -> impl axum::response::IntoResponse {
    let posts = filter_posts_from_db(params).await;

    (
        HxReplaceUrl(format!("/posts?category={}", params.category).parse().unwrap()),
        posts.render_html()
    )
}
```

**HTMX Header**: `HX-Replace-Url: /posts?category=rust`

**Use when**: You want to update the URL without creating a history entry.

### 9. HxLocation - Client-Side Navigation

Navigate to a new location with optional parameters:

```rust
use axum_htmx::HxLocation;
use serde_json::json;

async fn submit_form() -> HxLocation {
    HxLocation::from_uri("/success")
}

// With swap options
async fn navigate_with_options() -> HxLocation {
    HxLocation::from_value(json!({
        "path": "/posts/42",
        "target": "#main",
        "swap": "innerHTML"
    }))
}
```

**HTMX Header**: `HX-Location: /success` or `HX-Location: {"path": "/posts/42", ...}`

**Use when**: You need more control than a simple redirect.

## acton-htmx Extensions

### 10. HxSwapOob - Out-of-Band Swaps

Update multiple elements in a single response:

```rust
use acton_htmx::htmx::{HxSwapOob, SwapStrategy};

async fn update_post() -> impl axum::response::IntoResponse {
    let mut oob = HxSwapOob::new();

    // Update main content
    oob.add(
        "post-content",
        "<article><h1>Updated Post</h1></article>",
        SwapStrategy::InnerHTML
    );

    // Update notification badge
    oob.add(
        "notification-count",
        "<span class=\"badge\">5</span>",
        SwapStrategy::InnerHTML
    );

    // Update flash messages
    oob.add(
        "flash-messages",
        r#"<div class="alert success">Post updated!</div>"#,
        SwapStrategy::InnerHTML
    );

    oob
}
```

**HTML Output**:
```html
<div id="post-content" hx-swap-oob="innerHTML">
    <article><h1>Updated Post</h1></article>
</div>
<div id="notification-count" hx-swap-oob="innerHTML">
    <span class="badge">5</span>
</div>
<div id="flash-messages" hx-swap-oob="innerHTML">
    <div class="alert success">Post updated!</div>
</div>
```

**Swap Strategies**:
- `InnerHTML` - Replace inner HTML
- `OuterHTML` - Replace entire element
- `BeforeBegin` - Insert before element
- `AfterBegin` - Insert at start
- `BeforeEnd` - Insert at end
- `AfterEnd` - Insert after element

**Use when**: You need to update multiple page sections in one response.

## Combining Response Types

You can combine multiple response headers:

```rust
async fn complex_update() -> impl axum::response::IntoResponse {
    let content = Html("<div>Updated content</div>");

    (
        HxResponseTrigger::normal(["contentUpdated"]),
        HxPushUrl("/posts/42".parse().unwrap()),
        content
    )
}
```

## Automatic Template Rendering

Use `HxTemplate` trait for automatic partial/full page rendering:

```rust
use acton_htmx::prelude::*;
use askama::Template;

#[derive(Template)]
#[template(path = "posts/show.html")]
struct PostTemplate {
    post: Post,
}

async fn show_post(
    Path(id): Path<i64>,
    HxRequest(is_htmx): HxRequest,
) -> impl axum::response::IntoResponse {
    let post = load_post(id).await;

    PostTemplate { post }.render_htmx(is_htmx)
}
```

**How it works**:
- HTMX requests get just the `#main-content` section
- Regular browser requests get the full page with layout

See the [Template Guide](02-templates.md) for details.

## Error Handling

Return errors as HTMX responses:

```rust
use axum::http::StatusCode;

async fn save_post(form: Form<PostForm>) -> Result<HxRedirect, (StatusCode, Html<String>)> {
    match validate_and_save(form).await {
        Ok(_) => Ok(HxRedirect("/posts".parse().unwrap())),
        Err(errors) => {
            let html = render_errors(errors);
            Err((StatusCode::UNPROCESSABLE_ENTITY, Html(html)))
        }
    }
}
```

## Best Practices

### 1. Use the Right Response Type

- **Navigation**: Use `HxRedirect` or `HxLocation`
- **Refresh**: Use `HxRefresh` when the whole page changed
- **Notify**: Use `HxTrigger` to communicate with other elements
- **Update Multiple**: Use `HxSwapOob` for multi-element updates

### 2. Leverage Out-of-Band Swaps

Update navigation, notifications, and flash messages alongside main content:

```rust
async fn create_post(form: Form<PostForm>) -> impl axum::response::IntoResponse {
    let post = save_post(form).await;

    let mut response = HxSwapOob::new();

    // Main content
    response.add("main-content", post.render_html(), SwapStrategy::InnerHTML);

    // Flash message
    response.add(
        "flash-container",
        r#"<div class="success">Post created!</div>"#,
        SwapStrategy::InnerHTML
    );

    // Update post count
    let count = get_post_count().await;
    response.add(
        "post-count",
        &format!("<span>{count}</span>"),
        SwapStrategy::InnerHTML
    );

    response
}
```

### 3. Use Events for Coordination

Trigger events to update multiple independent components:

```rust
// In the handler
async fn update_cart(item: Form<CartItem>) -> impl axum::response::IntoResponse {
    add_to_cart(item).await;

    (
        HxResponseTrigger::normal(["cartUpdated"]),
        Html("<div>Item added</div>")
    )
}
```

```html
<!-- In templates -->
<div id="cart-icon" hx-get="/cart/count" hx-trigger="cartUpdated from:body">
    <span>🛒 0</span>
</div>

<div id="cart-total" hx-get="/cart/total" hx-trigger="cartUpdated from:body">
    <span>$0.00</span>
</div>
```

### 4. Maintain Browser History

Use `HxPushUrl` for navigable content:

```rust
async fn show_tab(Path(tab): Path<String>) -> impl axum::response::IntoResponse {
    let content = load_tab_content(&tab).await;

    (
        HxPushUrl(format!("/dashboard/{tab}").parse().unwrap()),
        content.render_html()
    )
}
```

## Next Steps

- **[Template Guide](02-templates.md)** - Learn template integration
- **[Form Handling](04-forms.md)** - Build validated forms with HTMX
- **[Examples](../examples/)** - See complete working examples

## Reference

- [HTMX Documentation](https://htmx.org/docs/)
- [axum-htmx Documentation](https://docs.rs/axum-htmx)
- [acton-htmx API Docs](../../target/doc/acton_htmx/htmx/)

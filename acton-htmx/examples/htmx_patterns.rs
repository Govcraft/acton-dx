//! Comprehensive HTMX patterns example
//!
//! Demonstrates all HTMX response types, extractors, and patterns.
//!
//! Run with:
//! ```bash
//! cargo run --example htmx_patterns
//! ```
//!
//! Then visit http://127.0.0.1:3000 in your browser.

use acton_htmx::{observability, state::ActonHtmxState};
use axum::{
    extract::Path,
    response::Html,
    routing::{delete, get, post, put},
    Router,
};
use axum_htmx::{
    HxBoosted, HxCurrentUrl, HxEvent, HxLocation, HxPrompt, HxPushUrl, HxRedirect, HxRefresh,
    HxRequest, HxResponseTrigger, HxReswap, HxRetarget, HxTarget, SwapOption,
};
use serde_json::json;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize observability
    observability::init()?;

    // Create application state
    let state = ActonHtmxState::new().await?;

    // Build router with all HTMX pattern examples
    let app = Router::new()
        .route("/", get(index))
        .route("/partial-content", get(partial_content))
        .route("/full-page", get(full_page))
        .route("/redirect-example", post(redirect_example))
        .route("/trigger-event", post(trigger_event))
        .route("/trigger-with-data", post(trigger_with_data))
        .route("/multiple-triggers", post(multiple_triggers))
        .route("/push-url-example", get(push_url_example))
        .route("/reswap-example", get(reswap_example))
        .route("/retarget-example", get(retarget_example))
        .route("/location-example", post(location_example))
        .route("/refresh-example", post(refresh_example))
        .route("/boosted-link", get(boosted_link))
        .route("/prompt-example", post(prompt_example))
        .route("/current-url-example", get(current_url_example))
        .route("/delete-item/:id", delete(delete_item))
        .route("/edit-item/:id", put(edit_item))
        .with_state(state);

    // Start server
    info!("Starting HTMX patterns server on http://127.0.0.1:3000");
    info!("Visit http://127.0.0.1:3000 to see all HTMX patterns");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Index page with links to all examples
async fn index() -> Html<&'static str> {
    Html(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>HTMX Patterns - acton-htmx</title>
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>
    <style>
        body { font-family: system-ui; max-width: 800px; margin: 40px auto; padding: 0 20px; }
        h1 { color: #2563eb; }
        h2 { color: #4b5563; margin-top: 2rem; }
        .example { background: #f3f4f6; padding: 1rem; margin: 1rem 0; border-radius: 8px; }
        button { background: #2563eb; color: white; padding: 8px 16px; border: none;
                 border-radius: 4px; cursor: pointer; margin: 4px; }
        button:hover { background: #1d4ed8; }
        #result { background: #dbeafe; padding: 1rem; margin-top: 1rem; border-radius: 4px; }
        .item { padding: 8px; margin: 4px 0; background: white; border-radius: 4px; }
    </style>
</head>
<body>
    <h1>🚀 HTMX Patterns in acton-htmx</h1>
    <p>Interactive examples of all HTMX response types and extractors.</p>

    <div id="result">Results will appear here...</div>

    <h2>1. Partial vs Full Page Content</h2>
    <div class="example">
        <p>HxRequest extractor automatically detects HTMX requests:</p>
        <button hx-get="/partial-content" hx-target="#result">Load Partial (HTMX)</button>
        <a href="/full-page"><button>Load Full Page (Regular Link)</button></a>
    </div>

    <h2>2. HX-Redirect</h2>
    <div class="example">
        <p>Client-side redirect via HTMX:</p>
        <button hx-post="/redirect-example" hx-target="#result">Redirect to Home</button>
    </div>

    <h2>3. HX-Trigger Events</h2>
    <div class="example">
        <p>Trigger client-side events:</p>
        <button hx-post="/trigger-event" hx-target="#result">Simple Trigger</button>
        <button hx-post="/trigger-with-data" hx-target="#result">Trigger with Data</button>
        <button hx-post="/multiple-triggers" hx-target="#result">Multiple Triggers</button>
        <div id="event-log"></div>
    </div>

    <h2>4. HX-Push-Url</h2>
    <div class="example">
        <p>Update browser URL without navigation:</p>
        <button hx-get="/push-url-example" hx-target="#result">Push URL</button>
    </div>

    <h2>5. HX-Reswap</h2>
    <div class="example">
        <p>Change swap strategy dynamically:</p>
        <div id="swap-target">
            <p>Original content</p>
            <button hx-get="/reswap-example" hx-target="#swap-target">Replace Outer HTML</button>
        </div>
    </div>

    <h2>6. HX-Retarget</h2>
    <div class="example">
        <p>Change target element dynamically:</p>
        <button hx-get="/retarget-example" hx-target="#original-target">Click to Retarget</button>
        <div id="original-target">Original target</div>
        <div id="new-target" style="background: #fef3c7; padding: 1rem; margin-top: 1rem;">
            New target (will receive content)
        </div>
    </div>

    <h2>7. HX-Location</h2>
    <div class="example">
        <p>Navigate with context data:</p>
        <button hx-post="/location-example" hx-target="#result">Navigate with Context</button>
    </div>

    <h2>8. HX-Refresh</h2>
    <div class="example">
        <p>Refresh the page:</p>
        <button hx-post="/refresh-example" hx-target="#result">Trigger Refresh</button>
    </div>

    <h2>9. Boosted Links</h2>
    <div class="example">
        <p>Detect boosted links:</p>
        <a href="/boosted-link" hx-boost="true" hx-target="#result">Boosted Link</a>
    </div>

    <h2>10. HX-Prompt</h2>
    <div class="example">
        <p>Access prompt values:</p>
        <button hx-post="/prompt-example"
                hx-prompt="Enter your name"
                hx-target="#result">Prompt Example</button>
    </div>

    <h2>11. HX-Current-URL</h2>
    <div class="example">
        <p>Access current URL:</p>
        <button hx-get="/current-url-example" hx-target="#result">Show Current URL</button>
    </div>

    <h2>12. CRUD with HTMX</h2>
    <div class="example">
        <p>RESTful operations:</p>
        <div id="items">
            <div class="item">
                Item 1
                <button hx-put="/edit-item/1" hx-target="#result">Edit</button>
                <button hx-delete="/delete-item/1" hx-target="#result"
                        hx-confirm="Delete this item?">Delete</button>
            </div>
        </div>
    </div>

    <script>
        // Listen for custom events
        document.body.addEventListener('myEvent', () => {
            document.getElementById('event-log').innerHTML =
                '<p style="color: green;">✓ myEvent triggered!</p>';
        });

        document.body.addEventListener('dataEvent', (e) => {
            document.getElementById('event-log').innerHTML =
                '<p style="color: green;">✓ dataEvent with: ' +
                JSON.stringify(e.detail) + '</p>';
        });

        document.body.addEventListener('event1', () => console.log('event1 fired'));
        document.body.addEventListener('event2', () => console.log('event2 fired'));
    </script>
</body>
</html>
    "#,
    )
}

/// Demonstrates automatic partial content detection
async fn partial_content(HxRequest(is_htmx): HxRequest) -> Html<String> {
    debug!("partial_content called, is_htmx={}", is_htmx);

    if is_htmx {
        // Return just the fragment
        Html("<p><strong>Partial Content</strong>: This is just the fragment (HTMX detected)</p>".to_string())
    } else {
        // Return full page
        Html("<html><body><h1>Full Page</h1><p>This is a complete HTML page</p></body></html>".to_string())
    }
}

/// Full page response
async fn full_page() -> Html<&'static str> {
    Html("<html><body><h1>Full Page Load</h1><p>This is a complete page reload (non-HTMX)</p></body></html>")
}

/// Demonstrates HxRedirect
async fn redirect_example() -> (HxRedirect, Html<&'static str>) {
    info!("Redirecting via HX-Redirect");
    (
        HxRedirect::from("/"),
        Html("<p>Redirecting...</p>"),
    )
}

/// Demonstrates simple HxResponseTrigger
async fn trigger_event() -> (HxResponseTrigger, Html<&'static str>) {
    info!("Triggering myEvent");
    (
        HxResponseTrigger::normal(["myEvent"]),
        Html("<p>Event triggered! (check event log)</p>"),
    )
}

/// Demonstrates HxResponseTrigger with data
async fn trigger_with_data() -> (HxResponseTrigger, Html<&'static str>) {
    info!("Triggering dataEvent with payload");
    let data = json!({"message": "Hello from server", "timestamp": 1234567890});
    let event = HxEvent::new_with_data("dataEvent", data).expect("valid JSON");
    (
        HxResponseTrigger::normal([event]),
        Html("<p>Event with data triggered!</p>"),
    )
}

/// Demonstrates multiple triggers
async fn multiple_triggers() -> (HxResponseTrigger, HxResponseTrigger, Html<&'static str>) {
    info!("Triggering multiple events");
    (
        HxResponseTrigger::normal(["event1", "event2"]),
        HxResponseTrigger::after_settle(["settleEvent"]),
        Html("<p>Multiple events triggered! (check console)</p>"),
    )
}

/// Demonstrates HxPushUrl
async fn push_url_example() -> (HxPushUrl, Html<&'static str>) {
    info!("Pushing URL /example/page");
    (
        HxPushUrl::from("/example/page"),
        Html("<p>URL pushed to /example/page (check address bar)</p>"),
    )
}

/// Demonstrates HxReswap
async fn reswap_example() -> (HxReswap, Html<&'static str>) {
    info!("Reswapping with outerHTML");
    (
        HxReswap::from(SwapOption::OuterHtml),
        Html(r#"<div id="swap-target" style="background: #d1fae5; padding: 1rem;">
            <p>✓ Outer HTML replaced!</p>
            <button hx-get="/reswap-example" hx-target="#swap-target">Replace Again</button>
        </div>"#),
    )
}

/// Demonstrates HxRetarget
async fn retarget_example() -> (HxRetarget, Html<&'static str>) {
    info!("Retargeting to #new-target");
    (
        HxRetarget::from("#new-target"),
        Html("<p>✓ Content delivered to retargeted element!</p>"),
    )
}

/// Demonstrates HxLocation
async fn location_example() -> (HxLocation, Html<&'static str>) {
    info!("Navigating with location");
    (
        HxLocation::from("/"),
        Html("<p>Location header sent</p>"),
    )
}

/// Demonstrates HxRefresh
async fn refresh_example() -> (HxRefresh, Html<&'static str>) {
    info!("Triggering page refresh");
    (
        HxRefresh::from(true),
        Html("<p>Page will refresh</p>"),
    )
}

/// Demonstrates HxBoosted extractor
async fn boosted_link(HxBoosted(is_boosted): HxBoosted) -> Html<String> {
    debug!("boosted_link called, is_boosted={}", is_boosted);

    if is_boosted {
        Html("<p><strong>Boosted:</strong> This link was boosted by HTMX</p>".to_string())
    } else {
        Html("<p>Regular link (not boosted)</p>".to_string())
    }
}

/// Demonstrates HxPrompt extractor
async fn prompt_example(HxPrompt(prompt_value): HxPrompt) -> Html<String> {
    if let Some(name) = prompt_value {
        info!("Received prompt value: {}", name);
        Html(format!(
            "<p>Hello, <strong>{}</strong>! (from prompt)</p>",
            name
        ))
    } else {
        Html("<p>No prompt value received</p>".to_string())
    }
}

/// Demonstrates HxCurrentUrl extractor
async fn current_url_example(HxCurrentUrl(url): HxCurrentUrl) -> Html<String> {
    if let Some(current_url) = url {
        debug!("Current URL: {}", current_url);
        Html(format!(
            "<p>Current URL: <code>{}</code></p>",
            current_url
        ))
    } else {
        Html("<p>No current URL available</p>".to_string())
    }
}

/// Demonstrates DELETE request
async fn delete_item(
    Path(id): Path<u32>,
    HxTarget(target): HxTarget,
) -> (HxResponseTrigger, Html<String>) {
    info!("Deleting item {}", id);
    debug!("Target element: {:?}", target);

    (
        HxResponseTrigger::normal(["itemDeleted"]),
        Html(format!("<p>✓ Item {} deleted</p>", id)),
    )
}

/// Demonstrates PUT request
async fn edit_item(Path(id): Path<u32>) -> Html<String> {
    info!("Editing item {}", id);
    Html(format!(
        "<p>✓ Item {} updated (edit form would go here)</p>",
        id
    ))
}

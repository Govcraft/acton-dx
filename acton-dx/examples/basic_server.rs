//! Basic acton-dx server example
//!
//! Demonstrates:
//! - Configuration loading
//! - Observability initialization
//! - Application state creation with agent runtime
//! - Session middleware integration
//! - Basic HTMX handler
//!
//! Run with: `cargo run --example basic_server`

use acton_dx::{observability, prelude::*};
use acton_service::prelude::ActonApp;
use axum::{extract::State, routing::get, Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize observability (logging, tracing)
    observability::init()?;

    tracing::info!("Starting acton-dx basic server");

    // Launch the Acton runtime - keep ownership in main for shutdown orchestration
    let mut runtime = ActonApp::launch();

    // Create application state (spawns session manager agent)
    let state = ActonHtmxState::new(&mut runtime).await?;

    tracing::info!(
        timeout_ms = state.config().htmx.request_timeout_ms,
        csrf_enabled = state.config().security.csrf_enabled,
        "Configuration loaded"
    );

    // Build router with HTMX routes and session middleware
    let app = Router::new()
        .route("/", get(index))
        .route("/about", get(about))
        .layer(SessionLayer::new(&state))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    tracing::info!("Server listening on http://127.0.0.1:3000");

    axum::serve(listener, app).await?;

    // Shutdown the agent runtime after server stops
    runtime.shutdown_all().await?;

    Ok(())
}

/// Index handler - demonstrates HxRequest extractor
async fn index(State(state): State<ActonHtmxState>, HxRequest(is_htmx): HxRequest) -> &'static str {
    tracing::debug!(
        is_htmx,
        timeout_ms = state.config().htmx.request_timeout_ms,
        "Handling index request"
    );

    if is_htmx {
        // Return partial for HTMX requests
        "<div id=\"content\">
    <h1>Welcome to acton-dx!</h1>
    <p>This is a partial response (HTMX request detected).</p>
    <a href=\"/about\" hx-get=\"/about\" hx-target=\"#content\">About</a>
</div>"
    } else {
        // Return full page for normal requests
        "<!DOCTYPE html>
<html>
<head>
    <title>acton-dx Example</title>
    <script src=\"https://unpkg.com/htmx.org@1.9.10\"></script>
</head>
<body>
    <div id=\"content\">
        <h1>Welcome to acton-dx!</h1>
        <p>This is a full page response.</p>
        <a href=\"/about\" hx-get=\"/about\" hx-target=\"#content\">About</a>
    </div>
</body>
</html>"
    }
}

/// About handler - demonstrates basic HTMX pattern
async fn about(HxRequest(is_htmx): HxRequest) -> &'static str {
    tracing::debug!(is_htmx, "Handling about request");

    if is_htmx {
        "<div id=\"content\">
    <h1>About acton-dx</h1>
    <p>A production-grade Rust web framework for HTMX applications.</p>
    <a href=\"/\" hx-get=\"/\" hx-target=\"#content\">Home</a>
</div>"
    } else {
        "<!DOCTYPE html>
<html>
<head>
    <title>About - acton-dx</title>
    <script src=\"https://unpkg.com/htmx.org@1.9.10\"></script>
</head>
<body>
    <div id=\"content\">
        <h1>About acton-dx</h1>
        <p>A production-grade Rust web framework for HTMX applications.</p>
        <a href=\"/\" hx-get=\"/\" hx-target=\"#content\">Home</a>
    </div>
</body>
</html>"
    }
}

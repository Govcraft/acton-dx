//! Simple HTMX example demonstrating corrected API usage for axum-htmx 0.8.1
//!
//! This example shows the correct usage of axum-htmx 0.8.1 API:
//! - HxRedirect::from() instead of ::to()
//! - HxResponseTrigger instead of HxTrigger
//! - HxRefresh::from(true) instead of bare HxRefresh
//! - HxEvent for event data
//!
//! Run with:
//! ```bash
//! cargo run --example simple_htmx
//! ```

use acton_htmx::{observability, state::ActonHtmxState};
use axum::{
    response::Html,
    routing::{get, post},
    Router,
};
use axum_htmx::{HxEvent, HxRedirect, HxRefresh, HxRequest, HxResponseTrigger};
use serde_json::json;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize observability
    observability::init()?;

    // Create application state
    let state = ActonHtmxState::new().await?;

    // Build router
    let app = Router::new()
        .route("/", get(index))
        .route("/partial", get(partial))
        .route("/redirect", post(redirect))
        .route("/trigger", post(trigger))
        .route("/refresh", post(refresh))
        .with_state(state);

    // Start server
    info!("Starting simple HTMX server on http://127.0.0.1:3000");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("simple_htmx.html"))
}

async fn partial(HxRequest(is_htmx): HxRequest) -> Html<String> {
    if is_htmx {
        Html("<p>This is a partial response (HTMX detected)</p>".to_string())
    } else {
        Html("<html><body><h1>Full Page</h1></body></html>".to_string())
    }
}

async fn redirect() -> (HxRedirect, Html<&'static str>) {
    info!("Redirecting...");
    (HxRedirect::from("/"), Html("<p>Redirecting...</p>"))
}

async fn trigger() -> (HxResponseTrigger, Html<&'static str>) {
    info!("Triggering event");
    let event =
        HxEvent::new_with_data("myEvent", json!({"message": "Hello!"})).expect("valid JSON");
    (
        HxResponseTrigger::normal([event]),
        Html("<p>Event triggered!</p>"),
    )
}

async fn refresh() -> (HxRefresh, Html<&'static str>) {
    info!("Triggering refresh");
    (HxRefresh::from(true), Html("<p>Refreshing...</p>"))
}

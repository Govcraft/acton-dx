//! HTMX + Templates Integration Example
//!
//! This example demonstrates the acton-htmx workflow:
//! - HxTemplate trait for automatic partial detection
//! - HxSwapOob for multi-target updates
//! - Template helpers
//!
//! Run with: `cargo run --example htmx_templates`

use acton_htmx::htmx::{HxRequest, HxSwapOob, SwapStrategy};
use acton_htmx::template::HxTemplate;
use askama::Template;
use axum::{
    response::{Html, IntoResponse, Response},
    routing::get,
    Form, Router,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

// =============================================================================
// Application State
// =============================================================================

#[derive(Clone)]
struct AppState {
    items: Arc<Mutex<Vec<Item>>>,
    counter: Arc<Mutex<i32>>,
}

#[allow(dead_code)]
#[derive(Clone)]
struct Item {
    id: usize,
    name: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            items: Arc::new(Mutex::new(vec![
                Item { id: 1, name: "First item".to_string() },
                Item { id: 2, name: "Second item".to_string() },
            ])),
            counter: Arc::new(Mutex::new(2)),
        }
    }
}

// =============================================================================
// Templates (simple inline for demonstration)
// =============================================================================

/// Simple template demonstrating HxTemplate trait
#[derive(Template)]
#[template(source = "<h1>{{ title }}</h1><p>Count: {{ count }}</p>", ext = "html")]
struct SimpleTemplate {
    title: String,
    count: i32,
}

/// Item list template
#[derive(Template)]
#[template(source = "{% for item in items %}<li>{{ item.name }}</li>{% endfor %}", ext = "html")]
struct ItemListTemplate {
    items: Vec<Item>,
}

/// Single item template
#[derive(Template)]
#[template(source = "<li id=\"item-{{ id }}\">{{ name }}</li>", ext = "html")]
struct ItemTemplate {
    id: usize,
    name: String,
}

/// Flash message template
#[derive(Template)]
#[template(source = "<div class=\"flash\">{{ message }}</div>", ext = "html")]
struct FlashTemplate {
    message: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// Index handler - demonstrates automatic partial detection
async fn index(
    axum::extract::State(state): axum::extract::State<AppState>,
    HxRequest(is_htmx): HxRequest,
) -> Response {
    let counter = *state.counter.lock().unwrap();

    let template = SimpleTemplate {
        title: "HTMX Templates Demo".to_string(),
        count: counter,
    };

    // HxTemplate::render_htmx automatically handles partial vs full page
    template.render_htmx(is_htmx)
}

/// Items list handler
async fn items_list(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let items = state.items.lock().unwrap().clone();
    let template = ItemListTemplate { items };
    template.render_html()
}

/// Form data for adding items
#[derive(Deserialize)]
struct AddItemForm {
    name: String,
}

/// Add item handler - demonstrates HxSwapOob multi-target updates
async fn add_item(
    axum::extract::State(state): axum::extract::State<AppState>,
    Form(form): Form<AddItemForm>,
) -> impl IntoResponse {
    // Create new item and get counter value
    let new_id = {
        let mut counter = state.counter.lock().unwrap();
        *counter += 1;
        usize::try_from(*counter).unwrap_or(0)
    };

    let item = Item {
        id: new_id,
        name: form.name.clone(),
    };

    // Add to list
    state.items.lock().unwrap().push(item);

    // Build multi-target response using HxSwapOob
    let item_template = ItemTemplate {
        id: new_id,
        name: form.name,
    };

    let flash_template = FlashTemplate {
        message: format!("Item {new_id} added!"),
    };

    let flash_html = flash_template.render().unwrap();
    let counter_str = new_id.to_string();

    // Primary content goes to the hx-target, OOB updates go elsewhere
    HxSwapOob::with_primary(item_template.render().unwrap())
        .with("flash", flash_html, SwapStrategy::InnerHTML)
        .with("counter", counter_str, SwapStrategy::InnerHTML)
}

/// OOB swap demo - shows render_oob method
async fn oob_demo() -> impl IntoResponse {
    let template = FlashTemplate {
        message: "This was an OOB swap!".to_string(),
    };

    // render_oob wraps content with hx-swap-oob attribute
    template.render_oob("flash", None)
}

/// OOB string demo - shows render_oob_str for manual composition
async fn oob_string_demo() -> impl IntoResponse {
    let flash1 = FlashTemplate {
        message: "First message".to_string(),
    };
    let flash2 = FlashTemplate {
        message: "Second message".to_string(),
    };

    // Manually compose multiple OOB elements
    let oob1 = flash1.render_oob_str("flash1", None).unwrap();
    let oob2 = flash2.render_oob_str("flash2", Some("outerHTML")).unwrap();

    Html(format!("<p>Main content</p>{oob1}{oob2}"))
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let state = AppState::default();

    let app = Router::new()
        .route("/", get(index))
        .route("/items", get(items_list).post(add_item))
        .route("/oob", get(oob_demo))
        .route("/oob-string", get(oob_string_demo))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Server running at http://127.0.0.1:3000");
    println!();
    println!("Endpoints:");
    println!("  GET  /           - Index (try with/without HX-Request header)");
    println!("  GET  /items      - List items");
    println!("  POST /items      - Add item (multi-target OOB response)");
    println!("  GET  /oob        - OOB swap demo");
    println!("  GET  /oob-string - Manual OOB composition demo");
    println!();
    println!("Test with curl:");
    println!("  curl http://localhost:3000/");
    println!("  curl -H 'HX-Request: true' http://localhost:3000/");
    println!("  curl -X POST -d 'name=NewItem' http://localhost:3000/items");

    axum::serve(listener, app).await.unwrap();
}

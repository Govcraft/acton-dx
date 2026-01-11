# Getting Started with acton-htmx

Welcome to acton-htmx! This guide will help you build your first HTMX-powered web application in Rust.

## What is acton-htmx?

acton-htmx is an opinionated Rust web framework that combines:
- **Axum's performance** - Fast, ergonomic web framework
- **HTMX's simplicity** - Build dynamic UIs with HTML, not JavaScript
- **Acton ecosystem** - Battle-tested configuration, observability, and actors

## Prerequisites

Before you begin, ensure you have:
- Rust 1.75+ installed (`rustup update`)
- PostgreSQL or SQLite database
- Basic familiarity with Rust and web concepts

## Installation

Install the acton-htmx CLI:

```bash
cargo install acton-htmx-cli
```

## Create Your First Project

Create a new project with the CLI:

```bash
acton-htmx new my-app
cd my-app
```

This generates a complete project structure:

```
my-app/
├── src/
│   ├── main.rs              # Application entry point
│   ├── handlers/            # Request handlers
│   │   ├── home.rs
│   │   └── auth.rs
│   └── models/              # Data models
│       └── user.rs
├── templates/               # Askama templates
│   ├── layouts/
│   │   ├── base.html       # Base HTML layout
│   │   └── app.html        # App layout with nav
│   ├── auth/               # Auth pages
│   │   ├── login.html
│   │   └── register.html
│   ├── partials/           # Reusable components
│   │   ├── nav.html
│   │   └── flash.html
│   └── home.html           # Welcome page
├── static/                 # CSS, JS, images
│   └── css/
│       └── app.css
├── config/                 # Configuration files
│   ├── development.toml
│   └── production.toml
├── migrations/             # SQLx migrations
│   └── 001_create_users.sql
└── Cargo.toml
```

## Set Up the Database

Create and migrate your database:

```bash
# PostgreSQL
createdb my_app_dev

# Run migrations
acton-htmx db migrate
```

For SQLite, the database file will be created automatically on first run.

## Start the Development Server

Start the server with hot reload:

```bash
acton-htmx dev
```

Visit http://localhost:3000 to see your app!

## Understanding the Code

### Main Entry Point (`src/main.rs`)

```rust
use acton_htmx::prelude::*;
use acton_reactive::prelude::ActonApp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize Acton actor runtime
    let mut runtime = ActonApp::launch();

    // Create application state (spawns session manager agent)
    let state = ActonHtmxState::new(&mut runtime).await?;

    // Build router with handlers
    let app = axum::Router::new()
        .route("/", axum::routing::get(home::index))
        .route("/login", axum::routing::get(login_form))
        .route("/login", axum::routing::post(login_post))
        .layer(SessionLayer::new(&state))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;

    // Cleanup on shutdown
    runtime.shutdown_all().await?;
    Ok(())
}
```

Key concepts:
1. **ActonApp runtime** - Powers background agents (sessions, CSRF)
2. **ActonHtmxState** - Application state with database, agents, config
3. **SessionLayer** - Middleware for session management
4. **Router** - Axum router with your handlers

### Request Handlers

Handlers use Axum extractors:

```rust
use acton_htmx::prelude::*;
use askama::Template;

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    user: Option<User>,
}

async fn index(
    HxRequest(is_htmx): HxRequest,
    auth: OptionalAuth<User>,
) -> impl axum::response::IntoResponse {
    let template = HomeTemplate {
        user: auth.user,
    };

    // Automatically renders partial for HTMX, full page otherwise
    template.render_htmx(is_htmx)
}
```

### Templates

Templates use Askama with HTMX attributes:

```html
<!-- templates/home.html -->
{% extends "layouts/app.html" %}

{% block content %}
<div id="main-content">
    <h1>Welcome{% if user %}, {{ user.email }}{% endif %}!</h1>

    <button
        hx-get="/posts"
        hx-target="#posts-list"
        hx-swap="innerHTML">
        Load Posts
    </button>

    <div id="posts-list"></div>
</div>
{% endblock %}
```

## Your First HTMX Handler

Add a simple HTMX endpoint:

```rust
// src/handlers/posts.rs
use acton_htmx::prelude::*;
use askama::Template;

#[derive(Template)]
#[template(source = r#"
    {% for post in posts %}
    <article class="post">
        <h3>{{ post.title }}</h3>
        <p>{{ post.body }}</p>
    </article>
    {% endfor %}
"#, ext = "html")]
struct PostsListTemplate {
    posts: Vec<Post>,
}

pub async fn list(
    State(state): State<ActonHtmxState>,
) -> impl axum::response::IntoResponse {
    let posts = sqlx::query_as!(
        Post,
        "SELECT id, title, body FROM posts ORDER BY created_at DESC"
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    PostsListTemplate { posts }.render_html()
}
```

Register the route:

```rust
let app = axum::Router::new()
    .route("/", axum::routing::get(home::index))
    .route("/posts", axum::routing::get(posts::list))  // Add this
    .layer(SessionLayer::new(&state))
    .with_state(state);
```

## Next Steps

Now that you have a working application:

1. **[HTMX Response Guide](01-htmx-responses.md)** - Learn all HTMX response types
2. **[Template Guide](02-templates.md)** - Master Askama integration
3. **[Authentication Guide](03-authentication.md)** - Add login and registration
4. **[Form Handling Guide](04-forms.md)** - Build validated forms
5. **[Deployment Guide](05-deployment.md)** - Deploy to production

## Common Commands

```bash
# Development
acton-htmx dev                    # Start dev server with hot reload
cargo watch -x check              # Continuous type checking

# Database
acton-htmx db migrate             # Run pending migrations
acton-htmx db reset               # Reset database (drop + create + migrate)
acton-htmx db create <name>       # Create new migration

# Testing
cargo test                        # Run all tests
cargo test --doc                  # Run doc tests
cargo clippy                      # Lint code

# Building
cargo build                       # Debug build
cargo build --release             # Release build
cargo check                       # Fast syntax check (no binary)

# Documentation
cargo doc --no-deps --open        # View API docs
```

## Getting Help

- **Examples**: See `examples/` directory in the repository
- **API Docs**: Run `cargo doc --no-deps --open`
- **Issues**: https://github.com/acton-htmx/acton-htmx/issues
- **Discussions**: https://github.com/acton-htmx/acton-htmx/discussions

## What's Next?

You now have a working acton-htmx application! The next guide will teach you about HTMX response types and how to build dynamic interfaces.

[Continue to HTMX Response Guide →](01-htmx-responses.md)

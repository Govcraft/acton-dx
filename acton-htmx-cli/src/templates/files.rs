//! Template file contents

// =============================================================================
// Cargo.toml Templates
// =============================================================================

/// Cargo.toml template for `SQLite` projects
pub const CARGO_TOML_SQLITE: &str = r#"[package]
name = "{{project_name}}"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# acton-htmx re-exports: tokio, axum, tower, tower-http, tracing, tracing-subscriber,
# serde, serde_json, sqlx, askama, validator, anyhow, thiserror, acton-reactive
acton-htmx = { version = "1.0.0-beta.7", default-features = false, features = ["sqlite"] }

[dev-dependencies]
http-body-util = "0.1"

[profile.dev]
opt-level = 0

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
"#;

/// Cargo.toml template for `PostgreSQL` projects
pub const CARGO_TOML_POSTGRES: &str = r#"[package]
name = "{{project_name}}"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# acton-htmx re-exports: tokio, axum, tower, tower-http, tracing, tracing-subscriber,
# serde, serde_json, sqlx, askama, validator, anyhow, thiserror, acton-reactive
acton-htmx = { version = "1.0.0-beta.7", default-features = false, features = ["postgres"] }

[dev-dependencies]
http-body-util = "0.1"

[profile.dev]
opt-level = 0

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
"#;

/// Cargo.toml template (backwards compatibility alias for `PostgreSQL`)
pub const CARGO_TOML: &str = CARGO_TOML_POSTGRES;

// =============================================================================
// README Templates
// =============================================================================

/// README.md template for `SQLite` projects (zero setup)
pub const README_MD_SQLITE: &str = r"# {{project_name}}

A web application built with [acton-htmx](https://github.com/govcraft/acton-htmx), an opinionated Rust framework for server-rendered HTMX applications.

## Quick Start

### Prerequisites

- Rust 1.75 or later
- acton-htmx CLI: `cargo install acton-htmx-cli`

### Setup

```bash
# Start development server (database created automatically!)
acton-htmx dev
```

Open http://localhost:3000

That's it! No database setup required. SQLite is used for development.

## Project Structure

```
{{project_name}}/
├── src/
│   ├── main.rs              # Application entry point
│   ├── handlers/            # HTTP request handlers
│   │   ├── mod.rs
│   │   ├── home.rs
│   │   └── auth.rs
│   └── models/              # Domain models
│       ├── mod.rs
│       └── user.rs
├── templates/               # Askama templates
│   ├── layouts/
│   ├── auth/
│   └── partials/
├── static/                  # Static assets
│   ├── css/
│   └── js/
├── config/                  # Configuration files
│   ├── development.toml
│   └── production.toml
├── data/                    # SQLite database files
│   └── dev.db              # Created on first run
└── migrations/              # Database migrations
```

## Development

### Running Tests

```bash
cargo test
```

### Database Commands

```bash
# Run migrations (automatic on startup)
acton-htmx db migrate

# Reset database
acton-htmx db reset

# Create new migration
acton-htmx db create <name>
```

### Building for Production

```bash
cargo build --release
```

## Switching to PostgreSQL

For production, you may want to use PostgreSQL. Recreate the project with:

```bash
acton-htmx new {{project_name}} --database postgres
```

Or modify `config/production.toml` and `Cargo.toml` manually.

## Features

- ✅ HTMX-first architecture with server-side rendering
- ✅ Session-based authentication with Argon2id
- ✅ CSRF protection enabled by default
- ✅ Security headers configured
- ✅ SQLite for zero-setup development
- ✅ Askama templates with compile-time checking
- ✅ Form validation with validator crate
- ✅ Flash messages via acton-reactive agents

## License

MIT
";

/// README.md template for `PostgreSQL` projects
pub const README_MD_POSTGRES: &str = r"# {{project_name}}

A web application built with [acton-htmx](https://github.com/govcraft/acton-htmx), an opinionated Rust framework for server-rendered HTMX applications.

## Quick Start

### Prerequisites

- Rust 1.75 or later
- PostgreSQL
- acton-htmx CLI: `cargo install acton-htmx-cli`

### Setup

1. Create database:
   ```bash
   createdb {{project_name_snake}}_dev
   ```

2. Run migrations:
   ```bash
   acton-htmx db migrate
   ```

3. Start development server:
   ```bash
   acton-htmx dev
   ```

4. Open http://localhost:3000

## Project Structure

```
{{project_name}}/
├── src/
│   ├── main.rs              # Application entry point
│   ├── handlers/            # HTTP request handlers
│   │   ├── mod.rs
│   │   ├── home.rs
│   │   └── auth.rs
│   └── models/              # Domain models
│       ├── mod.rs
│       └── user.rs
├── templates/               # Askama templates
│   ├── layouts/
│   ├── auth/
│   └── partials/
├── static/                  # Static assets
│   ├── css/
│   └── js/
├── config/                  # Configuration files
│   ├── development.toml
│   └── production.toml
└── migrations/              # Database migrations
```

## Development

### Running Tests

```bash
cargo test
```

### Database Commands

```bash
# Run migrations
acton-htmx db migrate

# Reset database
acton-htmx db reset

# Create new migration
acton-htmx db create <name>
```

### Building for Production

```bash
cargo build --release
```

## Features

- ✅ HTMX-first architecture with server-side rendering
- ✅ Session-based authentication with Argon2id
- ✅ CSRF protection enabled by default
- ✅ Security headers configured
- ✅ PostgreSQL with SQLx
- ✅ Askama templates with compile-time checking
- ✅ Form validation with validator crate
- ✅ Flash messages via acton-reactive agents

## License

MIT
";

/// README.md template (backwards compatibility alias for `PostgreSQL`)
pub const README_MD: &str = README_MD_POSTGRES;

/// .gitignore template for new projects
pub const GITIGNORE: &str = r"# Rust
/target
/Cargo.lock
**/*.rs.bk

# Environment
.env
.env.local

# Database
*.db
*.db-shm
*.db-wal

# IDE
.idea/
.vscode/
*.swp
*.swo
*~

# OS
.DS_Store
Thumbs.db

# Logs
*.log
";

// =============================================================================
// Configuration Templates
// =============================================================================

/// Development configuration template for `SQLite`
pub const CONFIG_DEV_SQLITE: &str = r#"# Development configuration (SQLite)

[server]
host = "127.0.0.1"
port = 3000

[database]
# SQLite database file - created automatically on first run
url = "sqlite:data/dev.db?mode=rwc"
max_connections = 5

[session]
secret = "development-secret-change-in-production"
cookie_name = "{{project_name_snake}}_session"
cookie_secure = false
max_age_seconds = 86400

[csrf]
enabled = true

[security_headers]
preset = "development"

[logging]
level = "debug"
"#;

/// Development configuration template for `PostgreSQL`
pub const CONFIG_DEV_POSTGRES: &str = r#"# Development configuration (PostgreSQL)

[server]
host = "127.0.0.1"
port = 3000

[database]
url = "postgres://localhost/{{project_name_snake}}_dev"
max_connections = 5

[session]
secret = "development-secret-change-in-production"
cookie_name = "{{project_name_snake}}_session"
cookie_secure = false
max_age_seconds = 86400

[csrf]
enabled = true

[security_headers]
preset = "development"

[logging]
level = "debug"
"#;

/// Production configuration template for `SQLite`
pub const CONFIG_PROD_SQLITE: &str = r#"# Production configuration (SQLite)
# Note: For production, consider using PostgreSQL for better performance and features

[server]
host = "0.0.0.0"
port = 3000

[database]
url = "${DATABASE_URL}"  # e.g., sqlite:/var/lib/myapp/prod.db?mode=rwc
max_connections = 10

[session]
secret = "${SESSION_SECRET}"
cookie_name = "{{project_name_snake}}_session"
cookie_secure = true
max_age_seconds = 86400

[csrf]
enabled = true

[security_headers]
preset = "strict"

[logging]
level = "info"
"#;

/// Production configuration template for `PostgreSQL`
pub const CONFIG_PROD_POSTGRES: &str = r#"# Production configuration (PostgreSQL)

[server]
host = "0.0.0.0"
port = 3000

[database]
url = "${DATABASE_URL}"
max_connections = 20

[session]
secret = "${SESSION_SECRET}"
cookie_name = "{{project_name_snake}}_session"
cookie_secure = true
max_age_seconds = 86400

[csrf]
enabled = true

[security_headers]
preset = "strict"

[logging]
level = "info"
"#;

/// Development configuration template (backwards compatibility alias for `PostgreSQL`)
pub const CONFIG_DEV: &str = CONFIG_DEV_POSTGRES;
/// Production configuration template (backwards compatibility alias for `PostgreSQL`)
pub const CONFIG_PROD: &str = CONFIG_PROD_POSTGRES;

// =============================================================================
// Main.rs Templates
// =============================================================================

/// Main.rs template for `SQLite` projects
pub const MAIN_RS_SQLITE: &str = r#"//! {{project_name}} - Built with acton-htmx

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]

use acton_htmx::prelude::*;
use acton_htmx::agents::{CsrfManagerAgent, SessionManagerAgent};
use acton_htmx::middleware::{SecurityHeadersConfig, SecurityHeadersLayer, SessionLayer};
use std::sync::Arc;

mod handlers;
mod models;

use handlers::{auth, home};

/// Application state with acton-reactive agents
#[derive(Clone)]
pub struct AppState {
    db: Arc<sqlx::SqlitePool>,
    session_manager: acton_reactive::prelude::AgentHandle,
    csrf_manager: acton_reactive::prelude::AgentHandle,
}

impl AppState {
    /// Create new application state, spawning all agents
    pub async fn new(
        runtime: &mut acton_reactive::prelude::AgentRuntime,
        db: sqlx::SqlitePool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            db: Arc::new(db),
            session_manager: SessionManagerAgent::spawn(runtime).await?,
            csrf_manager: CsrfManagerAgent::spawn(runtime).await?,
        })
    }

    /// Get the database pool
    #[must_use]
    pub fn db(&self) -> &sqlx::SqlitePool {
        &self.db
    }

    /// Get the session manager agent handle
    #[must_use]
    pub const fn session_manager(&self) -> &acton_reactive::prelude::AgentHandle {
        &self.session_manager
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "{{project_name_snake}}=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Ensure data directory exists for SQLite
    std::fs::create_dir_all("data")?;

    // Initialize SQLite database (created automatically with mode=rwc)
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite:data/dev.db?mode=rwc")
        .await?;

    // Run migrations automatically
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&db).await?;
    tracing::info!("Migrations complete!");

    // Launch acton-reactive runtime
    let mut runtime = acton_reactive::prelude::ActonApp::launch();

    // Create application state (spawns agents)
    let state = AppState::new(&mut runtime, db).await?;

    // Build router with session middleware using the agent handle
    let session_layer = SessionLayer::from_handle(state.session_manager().clone());

    let app = axum::Router::new()
        // Public routes
        .route("/", axum::routing::get(home::index))
        .route("/login", axum::routing::get(auth::login_form).post(auth::login))
        .route("/register", axum::routing::get(auth::register_form).post(auth::register))
        .route("/logout", axum::routing::post(auth::logout))
        // Static files
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        // Middleware
        .layer(SecurityHeadersLayer::new(SecurityHeadersConfig::development()))
        .layer(session_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        // State
        .with_state(state);

    // Start server
    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Starting server on http://{}", addr);

    axum::serve(listener, app).await?;

    // Shutdown agents gracefully
    runtime.shutdown_all().await?;

    Ok(())
}
"#;

/// Main.rs template for `PostgreSQL` projects
pub const MAIN_RS_POSTGRES: &str = r#"//! {{project_name}} - Built with acton-htmx

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]

use acton_htmx::prelude::*;
use acton_htmx::agents::{CsrfManagerAgent, SessionManagerAgent};
use acton_htmx::middleware::{SecurityHeadersConfig, SecurityHeadersLayer, SessionLayer};
use std::sync::Arc;

mod handlers;
mod models;

use handlers::{auth, home};

/// Application state with acton-reactive agents
#[derive(Clone)]
pub struct AppState {
    db: Arc<sqlx::PgPool>,
    session_manager: acton_reactive::prelude::AgentHandle,
    csrf_manager: acton_reactive::prelude::AgentHandle,
}

impl AppState {
    /// Create new application state, spawning all agents
    pub async fn new(
        runtime: &mut acton_reactive::prelude::AgentRuntime,
        db: sqlx::PgPool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            db: Arc::new(db),
            session_manager: SessionManagerAgent::spawn(runtime).await?,
            csrf_manager: CsrfManagerAgent::spawn(runtime).await?,
        })
    }

    /// Get the database pool
    #[must_use]
    pub fn db(&self) -> &sqlx::PgPool {
        &self.db
    }

    /// Get the session manager agent handle
    #[must_use]
    pub const fn session_manager(&self) -> &acton_reactive::prelude::AgentHandle {
        &self.session_manager
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "{{project_name_snake}}=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize PostgreSQL database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/{{project_name_snake}}_dev".to_string());
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&db).await?;
    tracing::info!("Migrations complete!");

    // Launch acton-reactive runtime
    let mut runtime = acton_reactive::prelude::ActonApp::launch();

    // Create application state (spawns agents)
    let state = AppState::new(&mut runtime, db).await?;

    // Build router with session middleware using the agent handle
    let session_layer = SessionLayer::from_handle(state.session_manager().clone());

    let app = axum::Router::new()
        // Public routes
        .route("/", axum::routing::get(home::index))
        .route("/login", axum::routing::get(auth::login_form).post(auth::login))
        .route("/register", axum::routing::get(auth::register_form).post(auth::register))
        .route("/logout", axum::routing::post(auth::logout))
        // Static files
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        // Middleware
        .layer(SecurityHeadersLayer::new(SecurityHeadersConfig::development()))
        .layer(session_layer)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        // State
        .with_state(state);

    // Start server
    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Starting server on http://{}", addr);

    axum::serve(listener, app).await?;

    // Shutdown agents gracefully
    runtime.shutdown_all().await?;

    Ok(())
}
"#;

/// Main.rs template (backwards compatibility alias for `PostgreSQL`)
pub const MAIN_RS: &str = MAIN_RS_POSTGRES;

/// Handlers module template
pub const HANDLERS_MOD: &str = r"//! HTTP request handlers

pub mod auth;
pub mod home;
";

/// Home handler template
pub const HANDLERS_HOME: &str = r#"//! Home page handlers

use askama::Template;
use axum::response::Html;

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate;

/// Home page
pub async fn index() -> Html<String> {
    Html(HomeTemplate.render().unwrap())
}
"#;

// =============================================================================
// Handler Templates
// =============================================================================

/// Authentication handler template (placeholder for user implementation)
pub const HANDLERS_AUTH_SQLITE: &str = r#"//! Authentication handlers
//!
//! This module provides placeholder authentication endpoints.
//! Implement your own User model and authentication logic as needed.

use askama::Template;
use axum::response::Html;

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
}

/// Show login form
pub async fn login_form() -> Html<String> {
    let template = LoginTemplate { error: None };
    Html(template.render().unwrap())
}

/// Show registration form
pub async fn register_form() -> Html<String> {
    let template = RegisterTemplate { error: None };
    Html(template.render().unwrap())
}
"#;

/// Authentication handler template for `PostgreSQL` (same as `SQLite` - uses framework User model)
pub const HANDLERS_AUTH_POSTGRES: &str = HANDLERS_AUTH_SQLITE;

/// Authentication handler template (backwards compatibility alias)
pub const HANDLERS_AUTH: &str = HANDLERS_AUTH_POSTGRES;

/// Models module template
pub const MODELS_MOD: &str = r"//! Domain models
//!
//! Add your application-specific models here.

// Example:
// pub mod user;
// pub mod post;
";

/// User model template (no longer needed as we use framework's User)
/// This constant is kept for backwards compatibility but generates an empty file
pub const MODELS_USER: &str = "//! User model\n\
//!\n\
//! The User model is provided by the acton-htmx framework.\n\
//! It includes:\n\
//! - Argon2id password hashing (OWASP recommended)\n\
//! - Email validation and normalization\n\
//! - Database operations (create, find_by_email, find_by_id, authenticate)\n\
//! - Role-based authorization support\n\
//! - Password strength validation\n\
//!\n\
//! See the acton-htmx documentation for full API:\n\
//! https://docs.rs/acton-htmx/latest/acton_htmx/auth/struct.User.html\n\
//!\n\
//! Example usage:\n\
//!\n\
//! ```rust,ignore\n\
//! use acton_htmx::auth::{User, EmailAddress, CreateUser};\n\
//! use sqlx::PgPool;\n\
//!\n\
//! // Create a new user with hashed password\n\
//! let email = EmailAddress::parse(\"user@example.com\")?;\n\
//! let create_user = CreateUser {\n\
//!     email,\n\
//!     password: \"SecurePass123\".to_string(),\n\
//! };\n\
//! let user = User::create(create_user, &pool).await?;\n\
//!\n\
//! // Authenticate a user\n\
//! let email = EmailAddress::parse(\"user@example.com\")?;\n\
//! let user = User::authenticate(&email, \"SecurePass123\", &pool).await?;\n\
//!\n\
//! // Verify password\n\
//! if user.verify_password(\"SecurePass123\")? {\n\
//!     println!(\"Password correct!\");\n\
//! }\n\
//!\n\
//! // Find user by email\n\
//! let user = User::find_by_email(&email, &pool).await?;\n\
//!\n\
//! // Find user by ID\n\
//! let user = User::find_by_id(user_id, &pool).await?;\n\
//! ```\n\
\n\
// Re-export from framework\n\
pub use acton_htmx::auth::User;\n";

/// Base HTML template
pub const TEMPLATE_BASE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}{{project_name}}{% endblock %}</title>

    <!-- HTMX -->
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>

    <!-- Styles -->
    <link rel="stylesheet" href="/static/css/app.css">

    {% block head %}{% endblock %}
</head>
<body>
    {% block body %}{% endblock %}
</body>
</html>
"#;

/// App layout template
pub const TEMPLATE_APP: &str = r#"{% extends "layouts/base.html" %}

{% block body %}
<div class="container">
    {% include "partials/nav.html" %}

    <div id="flash-messages">
        {% include "partials/flash.html" %}
    </div>

    <main>
        {% block content %}{% endblock %}
    </main>
</div>
{% endblock %}
"#;

/// Login page template
pub const TEMPLATE_LOGIN: &str = r#"{% extends "layouts/app.html" %}

{% block title %}Login - {{project_name}}{% endblock %}

{% block content %}
<div class="auth-form">
    <h1>Login</h1>

    {% if let Some(error) = error %}
    <div class="error">{{ error }}</div>
    {% endif %}

    <form hx-post="/login" hx-target="body">
        <div class="field">
            <label for="email">Email</label>
            <input type="email" id="email" name="email" required>
        </div>

        <div class="field">
            <label for="password">Password</label>
            <input type="password" id="password" name="password" required>
        </div>

        <button type="submit">Login</button>
    </form>

    <p>Don't have an account? <a href="/register">Register</a></p>
</div>
{% endblock %}
"#;

/// Registration page template
pub const TEMPLATE_REGISTER: &str = r#"{% extends "layouts/app.html" %}

{% block title %}Register - {{project_name}}{% endblock %}

{% block content %}
<div class="auth-form">
    <h1>Register</h1>

    {% if let Some(error) = error %}
    <div class="error">{{ error }}</div>
    {% endif %}

    <form hx-post="/register" hx-target="body">
        <div class="field">
            <label for="email">Email</label>
            <input type="email" id="email" name="email" required>
        </div>

        <div class="field">
            <label for="password">Password</label>
            <input type="password" id="password" name="password" required>
        </div>

        <div class="field">
            <label for="password_confirmation">Confirm Password</label>
            <input type="password" id="password_confirmation" name="password_confirmation" required>
        </div>

        <button type="submit">Register</button>
    </form>

    <p>Already have an account? <a href="/login">Login</a></p>
</div>
{% endblock %}
"#;

/// Flash messages partial template
pub const TEMPLATE_FLASH: &str = r"<!-- Flash messages will be rendered here -->
";

/// Navigation partial template
pub const TEMPLATE_NAV: &str = r#"<nav>
    <a href="/">Home</a>
    <a href="/login">Login</a>
    <a href="/register">Register</a>
</nav>
"#;

/// Home page template
pub const TEMPLATE_HOME: &str = r#"{% extends "layouts/app.html" %}

{% block title %}{{ title }} - {{project_name}}{% endblock %}

{% block content %}
<div class="home">
    <h1>{{ title }}</h1>
    <p>Welcome to your acton-htmx application!</p>

    <h2>Getting Started</h2>
    <ul>
        <li>Edit templates in <code>templates/</code></li>
        <li>Add handlers in <code>src/handlers/</code></li>
        <li>Define models in <code>src/models/</code></li>
        <li>Update routes in <code>src/main.rs</code></li>
    </ul>

    <h2>Documentation</h2>
    <ul>
        <li><a href="https://htmx.org" target="_blank">HTMX Documentation</a></li>
        <li><a href="https://github.com/govcraft/acton-htmx" target="_blank">acton-htmx Repository</a></li>
    </ul>
</div>
{% endblock %}
"#;

/// CSS stylesheet template
pub const STATIC_CSS: &str = r#"/* Basic styling for {{project_name}} */

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: system-ui, -apple-system, sans-serif;
    line-height: 1.6;
    color: #333;
    background: #f5f5f5;
}

.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
}

nav {
    background: white;
    padding: 1rem;
    margin-bottom: 2rem;
    border-radius: 4px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

nav a {
    margin-right: 1rem;
    text-decoration: none;
    color: #0066cc;
}

nav a:hover {
    text-decoration: underline;
}

main {
    background: white;
    padding: 2rem;
    border-radius: 4px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

.auth-form {
    max-width: 400px;
    margin: 0 auto;
}

.field {
    margin-bottom: 1rem;
}

label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 600;
}

input[type="text"],
input[type="email"],
input[type="password"] {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 1rem;
}

input.error {
    border-color: #dc3545;
}

.error {
    color: #dc3545;
    font-size: 0.875rem;
    margin-top: 0.25rem;
}

button[type="submit"] {
    width: 100%;
    padding: 0.75rem;
    background: #0066cc;
    color: white;
    border: none;
    border-radius: 4px;
    font-size: 1rem;
    cursor: pointer;
}

button[type="submit"]:hover {
    background: #0052a3;
}

code {
    background: #f5f5f5;
    padding: 0.2rem 0.4rem;
    border-radius: 3px;
    font-family: 'Courier New', monospace;
}

a {
    color: #0066cc;
}
"#;

// =============================================================================
// Migration Templates
// =============================================================================

/// Initial users table migration for `SQLite`
pub const MIGRATION_USERS_SQLITE: &str = r"-- Create users table (SQLite)

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
";

/// Initial users table migration for `PostgreSQL`
pub const MIGRATION_USERS_POSTGRES: &str = r"-- Create users table (PostgreSQL)

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX idx_users_email ON users(email);
";

/// Users migration template (backwards compatibility alias for `PostgreSQL`)
pub const MIGRATION_USERS: &str = MIGRATION_USERS_POSTGRES;

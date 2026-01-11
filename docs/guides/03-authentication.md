# Authentication & Security Guide

This guide covers session-based authentication, CSRF protection, and security best practices in acton-htmx.

## Overview

acton-htmx provides secure authentication out of the box:

- **Session-based auth** - HTTP-only cookies with secure defaults
- **Password hashing** - Argon2id with configurable parameters
- **CSRF protection** - Automatic token generation and validation
- **Security headers** - HSTS, CSP, X-Frame-Options, etc.
- **Actor-based session management** - Using acton-reactive

## Quick Start

The CLI-generated project includes complete authentication. Here's how it works:

### 1. User Model

```rust
use sqlx::FromRow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub password_hash: String,  // Never expose in responses!
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

### 2. Registration

```rust
use acton_htmx::prelude::*;
use validator::Validate;

#[derive(Debug, Validate, Deserialize)]
pub struct RegisterForm {
    #[validate(email)]
    pub email: String,

    #[validate(length(min = 8))]
    pub password: String,

    #[validate(must_match(other = "password"))]
    pub password_confirmation: String,
}

pub async fn register_post(
    State(state): State<ActonHtmxState>,
    mut session: SessionExtractor,
    Form(form): Form<RegisterForm>,
) -> Result<HxRedirect, AuthHandlerError> {
    // Validate input
    form.validate()?;

    // Check if email exists
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"
    )
    .bind(&form.email)
    .fetch_one(&state.db_pool)
    .await?;

    if exists {
        return Err(AuthHandlerError::EmailExists);
    }

    // Hash password
    let password_hash = hash_password(form.password.as_bytes())?;

    // Insert user
    let user_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id"
    )
    .bind(&form.email)
    .bind(&password_hash)
    .fetch_one(&state.db_pool)
    .await?;

    // Log user in
    session.set_user_id(Some(user_id));
    session.add_flash(FlashMessage::success("Welcome! Your account has been created."));

    Ok(HxRedirect("/dashboard".parse().unwrap()))
}
```

### 3. Login

```rust
#[derive(Debug, Validate, Deserialize)]
pub struct LoginForm {
    #[validate(email)]
    pub email: String,

    pub password: String,
}

pub async fn login_post(
    State(state): State<ActonHtmxState>,
    mut session: SessionExtractor,
    Form(form): Form<LoginForm>,
) -> Result<HxRedirect, AuthHandlerError> {
    // Validate input
    form.validate()?;

    // Find user
    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, password_hash, created_at FROM users WHERE email = $1"
    )
    .bind(&form.email)
    .fetch_optional(&state.db_pool)
    .await?;

    let user = match user {
        Some(u) => u,
        None => return Err(AuthHandlerError::InvalidCredentials),
    };

    // Verify password
    verify_password(form.password.as_bytes(), &user.password_hash)?;

    // Set session
    session.set_user_id(Some(user.id));
    session.add_flash(FlashMessage::success("Welcome back!"));

    Ok(HxRedirect("/dashboard".parse().unwrap()))
}
```

### 4. Logout

```rust
pub async fn logout_post(
    mut session: SessionExtractor,
) -> HxRedirect {
    session.set_user_id(None);
    session.add_flash(FlashMessage::info("You have been logged out."));

    HxRedirect("/".parse().unwrap())
}
```

## Password Security

### Password Hashing

acton-htmx uses Argon2id, the winner of the Password Hashing Competition:

```rust
use acton_htmx::auth::password::{hash_password, verify_password, PasswordHasher};

// Hash a password
let password = b"secure_password123";
let hash = hash_password(password)?;

// Verify a password
let is_valid = verify_password(password, &hash)?;
```

### Custom Hash Configuration

Configure Argon2 parameters for your security requirements:

```rust
use acton_htmx::auth::password::PasswordHashConfig;

let config = PasswordHashConfig {
    memory_cost: 65536,      // 64 MB
    time_cost: 3,            // 3 iterations
    parallelism: 4,          // 4 threads
};

let hasher = PasswordHasher::new(config);
let hash = hasher.hash_password(password)?;
```

### Password Requirements

Enforce strong passwords with validation:

```rust
use validator::Validate;

#[derive(Validate)]
struct PasswordForm {
    #[validate(length(min = 12))]
    #[validate(custom(function = "validate_password_strength"))]
    password: String,
}

fn validate_password_strength(password: &str) -> Result<(), validator::ValidationError> {
    let has_uppercase = password.chars().any(|c| c.is_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_numeric());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());

    if has_uppercase && has_lowercase && has_digit && has_special {
        Ok(())
    } else {
        Err(validator::ValidationError::new("weak_password"))
    }
}
```

## Session Management

### How Sessions Work

1. **Cookie-based** - Session ID stored in HTTP-only cookie
2. **Actor-managed** - `SessionManagerAgent` stores session data
3. **Secure defaults** - `SameSite=Lax`, `Secure` in production
4. **Automatic cleanup** - Expired sessions removed periodically

### Session Configuration

```rust
use acton_htmx::middleware::{SessionConfig, SessionLayer};
use std::time::Duration;

let session_config = SessionConfig {
    cookie_name: "session_id".to_string(),
    max_age: Duration::from_secs(7 * 24 * 60 * 60),  // 7 days
    secure: true,  // HTTPS only
    http_only: true,  // No JavaScript access
    same_site: axum::http::header::SameSite::Lax,
    domain: None,  // Current domain only
    path: "/".to_string(),
};

let app = Router::new()
    .route("/", get(index))
    .layer(SessionLayer::with_config(&state, session_config))
    .with_state(state);
```

### Using Sessions in Handlers

```rust
use acton_htmx::prelude::*;

async fn handler(mut session: SessionExtractor) -> impl axum::response::IntoResponse {
    // Get values
    let counter: Option<i32> = session.get("counter");

    // Set values
    session.set("counter".to_string(), counter.unwrap_or(0) + 1)?;

    // Remove values
    session.remove("old_key");

    // Check authentication
    if let Some(user_id) = session.user_id() {
        // User is logged in
    }

    Html("OK")
}
```

### Flash Messages

Flash messages persist for one request:

```rust
async fn save_post(
    mut session: SessionExtractor,
) -> HxRedirect {
    // Save post logic...

    session.add_flash(FlashMessage::success("Post saved successfully!"));

    HxRedirect("/posts".parse().unwrap())
}

async fn show_posts(
    mut session: SessionExtractor,
) -> impl axum::response::IntoResponse {
    let flash_messages = session.take_flashes();  // Clears them from session

    PostsTemplate { flash_messages }.render_html()
}
```

Flash levels:
- `FlashMessage::success(msg)` - Green, success icon
- `FlashMessage::error(msg)` - Red, error icon
- `FlashMessage::warning(msg)` - Yellow, warning icon
- `FlashMessage::info(msg)` - Blue, info icon

## Protected Routes

### Require Authentication

Use the `Authenticated` extractor to protect routes:

```rust
use acton_htmx::prelude::*;

async fn dashboard(
    Authenticated(user): Authenticated<User>,
) -> impl axum::response::IntoResponse {
    // This handler only runs if user is authenticated
    // Unauthenticated requests are redirected to /login

    DashboardTemplate { user }.render_html()
}
```

### Optional Authentication

Use `OptionalAuth` when authentication is optional:

```rust
async fn index(
    OptionalAuth(maybe_user): OptionalAuth<User>,
) -> impl axum::response::IntoResponse {
    // Handler runs for both authenticated and unauthenticated users

    HomeTemplate { user: maybe_user }.render_html()
}
```

### Custom Redirect

Configure where unauthenticated users are redirected:

```rust
// In middleware configuration
// TODO: Add configuration option for custom redirect URL
// For now, hardcoded to "/login" in the middleware
```

## CSRF Protection

### Automatic Protection

CSRF protection is enabled by default for state-changing methods (POST, PUT, DELETE):

```rust
use acton_htmx::middleware::CsrfLayer;

let app = Router::new()
    .route("/posts", post(create_post))
    .layer(CsrfLayer::new(&state))  // Validates CSRF tokens
    .with_state(state);
```

### Including CSRF Tokens

In forms:

```html
<form method="post" action="/posts">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">

    <!-- form fields -->
</form>
```

Pass token from handler:

```rust
use acton_htmx::extractors::CsrfToken;

async fn new_post(csrf: CsrfToken) -> impl axum::response::IntoResponse {
    NewPostTemplate {
        csrf_token: csrf.token().to_string(),
    }.render_html()
}
```

With HTMX:

```html
<form hx-post="/posts" hx-headers='{"X-CSRF-Token": "{{ csrf_token }}"}'>
    <!-- form fields -->
</form>
```

### Token Rotation

CSRF tokens are automatically rotated after each successful validation to prevent replay attacks.

## Security Headers

### Automatic Security Headers

acton-htmx sets secure headers by default:

```rust
use acton_htmx::middleware::SecurityHeadersLayer;

let app = Router::new()
    .route("/", get(index))
    .layer(SecurityHeadersLayer::new())  // Adds security headers
    .with_state(state);
```

Headers set:
- `X-Frame-Options: DENY` - Prevent clickjacking
- `X-Content-Type-Options: nosniff` - Prevent MIME sniffing
- `X-XSS-Protection: 1; mode=block` - Enable XSS filtering
- `Strict-Transport-Security: max-age=31536000` - Force HTTPS
- `Referrer-Policy: strict-origin-when-cross-origin` - Control referrer
- `Content-Security-Policy` - Configurable CSP

### Custom Content Security Policy

```rust
use acton_htmx::middleware::{SecurityHeadersLayer, CspConfig};

let csp = CspConfig {
    default_src: vec!["'self'"],
    script_src: vec!["'self'", "https://unpkg.com"],
    style_src: vec!["'self'", "'unsafe-inline'"],  // Needed for inline styles
    img_src: vec!["'self'", "data:", "https:"],
    connect_src: vec!["'self'"],
    font_src: vec!["'self'"],
    frame_ancestors: vec!["'none'"],
};

let app = Router::new()
    .route("/", get(index))
    .layer(SecurityHeadersLayer::with_csp(csp))
    .with_state(state);
```

## Best Practices

### 1. Never Log Sensitive Data

```rust
// BAD
tracing::info!("User logged in with password: {}", password);

// GOOD
tracing::info!("User logged in: {}", user.email);
```

### 2. Use HTTPS in Production

```toml
# config/production.toml
[server]
use_tls = true
cert_path = "/path/to/cert.pem"
key_path = "/path/to/key.pem"
```

### 3. Set Secure Cookie Attributes

```rust
let session_config = SessionConfig {
    secure: true,        // HTTPS only
    http_only: true,     // No JavaScript access
    same_site: SameSite::Strict,  // CSRF protection
    ..Default::default()
};
```

### 4. Validate All Input

```rust
use validator::Validate;

#[derive(Validate)]
struct UserInput {
    #[validate(email)]
    email: String,

    #[validate(length(min = 1, max = 100))]
    name: String,
}

async fn handler(Form(input): Form<UserInput>) -> Result<(), ValidationError> {
    input.validate()?;  // Validate before using
    // ... use input safely
}
```

### 5. Rate Limit Authentication Endpoints

```rust
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use std::time::Duration;

let governor_conf = GovernorConfigBuilder::default()
    .per_millisecond(1000)  // 1 request per second
    .burst_size(5)          // Allow 5 requests in burst
    .finish()
    .unwrap();

let app = Router::new()
    .route("/login", post(login_post))
    .layer(GovernorLayer { config: Box::leak(Box::new(governor_conf)) });
```

### 6. Hash Passwords on Registration

```rust
// ALWAYS hash passwords before storing
let password_hash = hash_password(form.password.as_bytes())?;

sqlx::query!(
    "INSERT INTO users (email, password_hash) VALUES ($1, $2)",
    email,
    password_hash  // Store hash, never plaintext!
)
.execute(&pool)
.await?;
```

### 7. Verify Passwords Securely

```rust
// Use constant-time comparison
match verify_password(input_password.as_bytes(), &user.password_hash) {
    Ok(true) => { /* Valid */ }
    Ok(false) | Err(_) => {
        // Don't reveal whether user exists or password is wrong
        return Err(AuthHandlerError::InvalidCredentials);
    }
}
```

## Advanced Topics

### Remember Me

Extend session duration for "remember me" functionality:

```rust
async fn login_with_remember_me(
    Form(form): Form<LoginForm>,
    mut session: SessionExtractor,
) -> Result<HxRedirect, AuthHandlerError> {
    // Verify credentials...

    if form.remember_me {
        // Extend session to 30 days
        session.set_max_age(Duration::from_secs(30 * 24 * 60 * 60));
    }

    session.set_user_id(Some(user.id));
    Ok(HxRedirect("/dashboard".parse().unwrap()))
}
```

### Email Verification

Send verification emails after registration:

```rust
async fn register_post(
    State(state): State<ActonHtmxState>,
    Form(form): Form<RegisterForm>,
) -> Result<HxRedirect, AuthHandlerError> {
    // Create user...

    // Generate verification token
    let token = generate_secure_token();

    sqlx::query!(
        "INSERT INTO email_verifications (user_id, token, expires_at) VALUES ($1, $2, $3)",
        user_id,
        token,
        Utc::now() + Duration::hours(24)
    )
    .execute(&state.db_pool)
    .await?;

    // Send email (using external service)
    send_verification_email(&form.email, &token).await?;

    Ok(HxRedirect("/verify-email".parse().unwrap()))
}
```

### Two-Factor Authentication (2FA)

Add TOTP-based 2FA:

```rust
// Store TOTP secret
sqlx::query!(
    "UPDATE users SET totp_secret = $1 WHERE id = $2",
    totp_secret,
    user_id
)
.execute(&pool)
.await?;

// Verify TOTP code
async fn verify_2fa(
    user_id: i64,
    code: &str,
) -> Result<bool, Error> {
    let user = get_user(user_id).await?;

    if let Some(secret) = user.totp_secret {
        let totp = TOTP::new(secret);
        Ok(totp.verify(code))
    } else {
        Ok(false)
    }
}
```

## Testing Authentication

### Test Password Hashing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = b"test_password";

        let hash = hash_password(password).unwrap();

        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password(b"wrong_password", &hash).unwrap());
    }
}
```

### Test Protected Routes

```rust
#[tokio::test]
async fn test_protected_route_requires_auth() {
    let app = test_app().await;

    let response = app
        .get("/dashboard")
        .await;

    // Should redirect to login
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get("Location").unwrap(), "/login");
}
```

## Next Steps

- **[Form Handling Guide](04-forms.md)** - Build validated forms
- **[Deployment Guide](05-deployment.md)** - Deploy securely to production
- **[Examples](../examples/)** - See complete auth examples

## Reference

- [Argon2 Documentation](https://docs.rs/argon2)
- [OWASP Authentication Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)
- [acton-htmx Auth API](../../target/doc/acton_htmx/auth/)

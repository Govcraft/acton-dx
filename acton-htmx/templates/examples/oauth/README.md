# OAuth2 Example Templates

This directory contains example templates demonstrating how to integrate OAuth2 authentication into your acton-htmx application.

## Overview

acton-htmx provides complete OAuth2 infrastructure out of the box:
- **Google OAuth2** with OpenID Connect
- **GitHub OAuth2**
- **Generic OIDC** provider support

The framework handles all the complexity:
- CSRF protection via state tokens
- PKCE (Proof Key for Code Exchange) for security
- Account linking (multiple providers per user)
- Automatic user creation
- Session management

## Usage

These templates are **examples** to help you get started. Copy them to your application's `templates/` directory and customize as needed.

### Files

- **`login.html`** - Login page with OAuth provider buttons
- **`account-settings.html`** - Settings page for managing linked OAuth accounts
- **`partials/`** - Reusable template fragments

### Configuration

First, configure OAuth2 in your `config.toml`:

```toml
[oauth2.google]
client_id = "your-google-client-id"
client_secret = "your-google-client-secret"
redirect_uri = "http://localhost:3000/auth/google/callback"
scopes = ["openid", "email", "profile"]

[oauth2.github]
client_id = "your-github-client-id"
client_secret = "your-github-client-secret"
redirect_uri = "http://localhost:3000/auth/github/callback"
scopes = ["read:user", "user:email"]
```

### Routes

Add OAuth2 routes to your application:

```rust
use acton_htmx::oauth2::handlers::{
    initiate_oauth, handle_oauth_callback, unlink_oauth_account
};
use axum::{Router, routing::get};

let app = Router::new()
    // Initiate OAuth flow
    .route("/auth/:provider", get(initiate_oauth))
    // OAuth callback (provider redirects here)
    .route("/auth/:provider/callback", get(handle_oauth_callback))
    // Unlink OAuth account
    .route("/auth/:provider/unlink", get(unlink_oauth_account));
```

### Database Migration

Run the OAuth2 migration to create the `oauth_accounts` table:

```bash
sqlx migrate run
```

The migration is included in the framework and creates:
- `oauth_accounts` table for storing provider linkages
- Foreign key to `users` table
- Unique constraint on (provider, provider_user_id)

## How It Works

### 1. User Clicks "Sign in with Google"

The user clicks a button in your login page:

```html
<a href="/auth/google" class="oauth-button">
    Sign in with Google
</a>
```

### 2. Framework Initiates OAuth Flow

The `initiate_oauth` handler:
1. Generates a CSRF state token
2. Generates a PKCE challenge
3. Stores both in the session
4. Redirects to Google's authorization page

### 3. User Authorizes at Google

The user logs in at Google and authorizes your app.

### 4. Google Redirects Back

Google redirects to `/auth/google/callback` with an authorization code and state token.

### 5. Framework Completes Authentication

The `handle_oauth_callback` handler:
1. Validates the CSRF state token
2. Exchanges the code for an access token (using PKCE)
3. Fetches user information from Google
4. Creates or links the OAuth account
5. Authenticates the user via session
6. Redirects to dashboard

## Security Features

### CSRF Protection

State tokens are:
- Generated using cryptographically secure random numbers
- Stored server-side in the OAuth2Agent
- Validated on callback
- Single-use (removed after validation)
- Expire after 10 minutes

### PKCE

All providers use PKCE (Proof Key for Code Exchange):
- Prevents authorization code interception attacks
- Generates a code verifier and challenge
- Verifier stored in session (HTTP-only cookie)
- Challenge sent to provider
- Verifier used during token exchange

### Account Linking

- Users can link multiple OAuth providers to one account
- Automatic linking if already authenticated
- Automatic user creation if new OAuth account
- Prevents duplicate accounts via unique constraint

## Customization

### Styling

The example templates use minimal inline styles. Add your own CSS:

```html
{% extends "base.html" %}

{% block styles %}
<link rel="stylesheet" href="/static/css/oauth.css">
{% endblock %}
```

### Error Handling

Customize error messages in your application code:

```rust
match handle_oauth_callback(state, provider, params, session).await {
    Ok(response) => response,
    Err(ActonHtmxError::Forbidden(_)) => {
        // Custom CSRF error page
    }
    Err(e) => {
        // Generic error handling
    }
}
```

### Redirects

The framework redirects to:
- `/dashboard` after successful authentication
- `/settings/accounts` after unlinking

Customize by modifying session state:

```rust
// Set custom return URL before initiating OAuth
session.set("return_url".to_string(), "/profile")?;
```

## Testing

Test OAuth flows locally using:
- Google OAuth2 test credentials (localhost redirect URIs allowed)
- GitHub OAuth Apps (localhost redirect URIs allowed)

For production, update redirect URIs to your domain.

## Production Checklist

- [ ] Environment variables for client secrets (never commit secrets)
- [ ] HTTPS redirect URIs in production
- [ ] Rate limiting on OAuth endpoints
- [ ] Logging and monitoring of OAuth flows
- [ ] Error tracking for failed authentications
- [ ] User-friendly error pages
- [ ] Privacy policy and terms of service links
- [ ] Clear explanation of what data you access

## Support

- Framework documentation: https://acton-htmx.dev/oauth2
- OAuth2 RFC: https://datatracker.ietf.org/doc/html/rfc6749
- Google OAuth2: https://developers.google.com/identity/protocols/oauth2
- GitHub OAuth: https://docs.github.com/en/developers/apps/building-oauth-apps

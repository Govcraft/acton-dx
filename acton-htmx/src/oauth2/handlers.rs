//! OAuth2 HTTP handlers
//!
//! This module provides Axum handlers for OAuth2 authentication flows:
//! - Initiate OAuth flow
//! - Handle OAuth callback
//! - Link OAuth account to existing user
//! - Unlink OAuth account

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect},
};
use acton_reactive::prelude::AgentHandleInterface;
use serde::Deserialize;
use sqlx::PgPool;

use crate::{
    auth::{password::hash_password, Session},
    error::ActonHtmxError,
    htmx::{HxRedirect, HxResponseTrigger},
    oauth2::{
        agent::{GenerateState, ValidateState, RemoveState},
        models::OAuthAccount,
        providers::{GitHubProvider, GoogleProvider, OidcProvider},
        types::{OAuthProvider, OAuthUserInfo, ProviderConfig},
    },
    state::ActonHtmxState,
};

/// OAuth2 callback query parameters
#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    /// Authorization code from provider
    pub code: String,
    /// CSRF state token
    pub state: String,
    /// Optional error from provider
    pub error: Option<String>,
    /// Optional error description
    pub error_description: Option<String>,
}

/// Initiate OAuth2 flow
///
/// This handler initiates the OAuth2 authorization code flow by:
/// 1. Generating a CSRF state token
/// 2. Storing the state token and PKCE verifier in session
/// 3. Redirecting to the provider's authorization endpoint
///
/// # Errors
///
/// Returns error if the provider is not configured or if state generation fails
pub async fn initiate_oauth(
    State(state): State<ActonHtmxState>,
    Path(provider_name): Path<String>,
    mut session: Session,
) -> Result<impl IntoResponse, ActonHtmxError> {
    // Parse provider
    let provider = provider_name.parse::<OAuthProvider>()
        .map_err(|_| ActonHtmxError::BadRequest(format!("Unknown provider: {provider_name}")))?;

    // Get OAuth2 config
    let oauth_config = &state.config().oauth2;

    let provider_config = oauth_config
        .get_provider(provider)
        .map_err(|e| ActonHtmxError::ServerError(format!("Provider not configured: {e}")))?;

    // Generate CSRF state token
    let (generate_msg, rx) = GenerateState::new(provider);
    state.oauth2_agent().send(generate_msg).await;
    let oauth_state = rx
        .await
        .map_err(|e| ActonHtmxError::ServerError(format!("Failed to generate state: {e}")))?;

    // Generate authorization URL and PKCE verifier
    let (auth_url, _csrf_state, pkce_verifier) = match provider {
        OAuthProvider::Google => {
            let google = GoogleProvider::new(provider_config)
                .map_err(|e| ActonHtmxError::ServerError(format!("Google OAuth error: {e}")))?;
            google.authorization_url()
        }
        OAuthProvider::GitHub => {
            let github = GitHubProvider::new(provider_config)
                .map_err(|e| ActonHtmxError::ServerError(format!("GitHub OAuth error: {e}")))?;
            github.authorization_url()
        }
        OAuthProvider::Oidc => {
            let oidc = OidcProvider::new(provider_config)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("OIDC error: {e}")))?;
            oidc.authorization_url()
        }
    };

    // Store state and PKCE verifier in session
    session.set("oauth2_state".to_string(), &oauth_state.token)?;
    session.set("oauth2_pkce_verifier".to_string(), &pkce_verifier)?;
    session.set("oauth2_provider".to_string(), &provider_name)?;

    // Redirect to provider's authorization endpoint
    Ok(Redirect::to(&auth_url))
}

/// Validate CSRF state token from session and OAuth2 agent
///
/// # Errors
///
/// Returns error if state token is missing, mismatched, or expired
async fn validate_oauth_state(
    state: &ActonHtmxState,
    session: &Session,
    params: &OAuthCallback,
    provider_name: &str,
) -> Result<(), ActonHtmxError> {
    // Validate CSRF state token from session
    let stored_state: String = session
        .get("oauth2_state")
        .ok_or_else(|| ActonHtmxError::BadRequest("No OAuth2 state in session".to_string()))?;

    if stored_state != params.state {
        tracing::warn!(
            provider = %provider_name,
            expected = %stored_state,
            received = %params.state,
            "OAuth2 state mismatch (potential CSRF attack)"
        );
        return Err(ActonHtmxError::Forbidden(
            "OAuth2 state mismatch".to_string(),
        ));
    }

    // Validate state with OAuth2 agent
    let (validate_msg, validate_rx) = ValidateState::new(params.state.clone());
    state.oauth2_agent().send(validate_msg).await;
    validate_rx
        .await
        .map_err(|e| ActonHtmxError::ServerError(format!("Failed to validate state: {e}")))?
        .ok_or_else(|| ActonHtmxError::Forbidden("Invalid or expired OAuth2 state".to_string()))?;

    // Remove state token (one-time use)
    state
        .oauth2_agent()
        .send(RemoveState {
            token: params.state.clone(),
        })
        .await;

    Ok(())
}

/// Exchange authorization code for access token and fetch user info
///
/// # Errors
///
/// Returns error if token exchange or user info fetch fails
async fn exchange_code_and_fetch_user(
    provider: OAuthProvider,
    provider_config: &ProviderConfig,
    code: &str,
    pkce_verifier: &str,
) -> Result<OAuthUserInfo, ActonHtmxError> {
    match provider {
        OAuthProvider::Google => {
            let google = GoogleProvider::new(provider_config)
                .map_err(|e| ActonHtmxError::ServerError(format!("Google OAuth error: {e}")))?;

            let token = google
                .exchange_code(code, pkce_verifier)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("Token exchange failed: {e}")))?;

            google
                .fetch_user_info(&token.access_token)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("Failed to fetch user info: {e}")))
        }
        OAuthProvider::GitHub => {
            let github = GitHubProvider::new(provider_config)
                .map_err(|e| ActonHtmxError::ServerError(format!("GitHub OAuth error: {e}")))?;

            let token = github
                .exchange_code(code, pkce_verifier)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("Token exchange failed: {e}")))?;

            github
                .fetch_user_info(&token.access_token)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("Failed to fetch user info: {e}")))
        }
        OAuthProvider::Oidc => {
            let oidc = OidcProvider::new(provider_config)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("OIDC error: {e}")))?;

            let token = oidc
                .exchange_code(code, pkce_verifier)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("Token exchange failed: {e}")))?;

            oidc
                .fetch_user_info(&token.access_token)
                .await
                .map_err(|e| ActonHtmxError::ServerError(format!("Failed to fetch user info: {e}")))
        }
    }
}

/// Find or create OAuth account and return user ID
///
/// # Errors
///
/// Returns error if database operations fail
async fn find_or_create_oauth_user(
    pool: &PgPool,
    session: &Session,
    provider: OAuthProvider,
    user_info: &OAuthUserInfo,
) -> Result<i64, ActonHtmxError> {
    let oauth_account =
        OAuthAccount::find_by_provider(pool, provider, &user_info.provider_user_id).await?;

    if let Some(mut account) = oauth_account {
        // Existing OAuth account - update info and use existing user_id
        account.update_info(pool, user_info).await?;
        Ok(account.user_id)
    } else if let Some(user_id) = session.get::<i64>("user_id") {
        // Link to existing authenticated user
        let account = OAuthAccount::link_account(pool, user_id, provider, user_info).await?;
        Ok(account.user_id)
    } else {
        // Create new user for this OAuth account
        let user_id = create_user_from_oauth(pool, &user_info.email).await?;
        let account = OAuthAccount::link_account(pool, user_id, provider, user_info).await?;
        Ok(account.user_id)
    }
}

/// Complete OAuth authentication by updating session
fn complete_oauth_authentication(
    session: &mut Session,
    user_id: i64,
) -> Result<(), ActonHtmxError> {
    session.set("user_id".to_string(), user_id)?;
    session.remove("oauth2_state");
    session.remove("oauth2_pkce_verifier");
    session.remove("oauth2_provider");
    Ok(())
}

/// Handle OAuth2 callback
///
/// This handler completes the OAuth2 authorization code flow by:
/// 1. Validating the CSRF state token
/// 2. Exchanging the authorization code for an access token
/// 3. Fetching user information from the provider
/// 4. Creating or linking the OAuth account
/// 5. Authenticating the user
///
/// # Errors
///
/// Returns error if state validation fails, token exchange fails, or database operations fail
pub async fn handle_oauth_callback(
    State(state): State<ActonHtmxState>,
    Path(provider_name): Path<String>,
    Query(params): Query<OAuthCallback>,
    mut session: Session,
) -> Result<impl IntoResponse, ActonHtmxError> {
    // Check for OAuth error
    if let Some(error) = params.error {
        let description = params.error_description.unwrap_or_default();
        tracing::warn!(
            provider = %provider_name,
            error = %error,
            description = %description,
            "OAuth2 error from provider"
        );
        return Err(ActonHtmxError::BadRequest(format!(
            "OAuth2 error: {error}"
        )));
    }

    // Parse provider
    let provider = provider_name.parse::<OAuthProvider>()
        .map_err(|_| ActonHtmxError::BadRequest(format!("Unknown provider: {provider_name}")))?;

    // Validate CSRF state token
    validate_oauth_state(&state, &session, &params, &provider_name).await?;

    // Get PKCE verifier from session
    let pkce_verifier: String = session
        .get("oauth2_pkce_verifier")
        .ok_or_else(|| ActonHtmxError::BadRequest("No PKCE verifier in session".to_string()))?;

    // Get OAuth2 config
    let provider_config = state.config().oauth2
        .get_provider(provider)
        .map_err(|e| ActonHtmxError::ServerError(format!("Provider not configured: {e}")))?;

    // Exchange code for token and fetch user info
    let user_info = exchange_code_and_fetch_user(
        provider,
        provider_config,
        &params.code,
        &pkce_verifier,
    )
    .await?;

    // Find or create OAuth account
    let pool = state.database_pool();
    let user_id = find_or_create_oauth_user(pool, &session, provider, &user_info).await?;

    // Authenticate user
    complete_oauth_authentication(&mut session, user_id)?;

    tracing::info!(
        provider = %provider_name,
        user_id = user_id,
        "User authenticated via OAuth2"
    );

    // Redirect to dashboard or return URL
    let return_url = session
        .get::<String>("return_url")
        .unwrap_or_else(|| "/dashboard".to_string());
    session.remove("return_url");

    Ok((HxRedirect(return_url), ()))
}

/// Unlink OAuth account
///
/// This handler unlinks an OAuth account from the currently authenticated user.
///
/// # Errors
///
/// Returns error if the user is not authenticated or if database operations fail
pub async fn unlink_oauth_account(
    State(state): State<ActonHtmxState>,
    Path(provider_name): Path<String>,
    session: Session,
) -> Result<impl IntoResponse, ActonHtmxError> {
    // Require authentication
    let user_id: i64 = session
        .get("user_id")
        .ok_or_else(|| ActonHtmxError::Unauthorized("Not authenticated".to_string()))?;

    // Parse provider
    let provider = provider_name.parse::<OAuthProvider>()
        .map_err(|_| ActonHtmxError::BadRequest(format!("Unknown provider: {provider_name}")))?;

    // Unlink account
    let pool = state.database_pool();
    let unlinked = OAuthAccount::unlink_account(pool, user_id, provider).await?;

    if unlinked {
        tracing::info!(
            provider = %provider_name,
            user_id = user_id,
            "OAuth account unlinked"
        );

        Ok((
            HxResponseTrigger::normal(vec!["oauth-account-unlinked"]),
            HxRedirect("/settings/accounts".to_string()),
            (),
        ))
    } else {
        Err(ActonHtmxError::NotFound(
            "OAuth account not found".to_string(),
        ))
    }
}

/// Create a new user from OAuth account
///
/// This is a helper function to create a new user when authenticating with OAuth
/// for the first time and no existing user is found.
///
/// # Errors
///
/// Returns error if database operations fail
async fn create_user_from_oauth(pool: &PgPool, email: &str) -> Result<i64, ActonHtmxError> {
    // Create user with a random password (OAuth-only users don't have passwords)
    // They can set a password later if they want email/password login
    use rand::Rng;
    let random_password: String = rand::rng()
        .sample_iter(rand::distr::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    // Hash the random password using Argon2id for security
    let password_hash = hash_password(&random_password)
        .map_err(|e| ActonHtmxError::ServerError(format!("Failed to hash password: {e}")))?;

    let user_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id",
    )
    .bind(email)
    .bind(&password_hash)
    .fetch_one(pool)
    .await?;

    Ok(user_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_callback_deserialize() {
        let json = r#"{"code": "abc123", "state": "xyz789"}"#;
        let callback: OAuthCallback = serde_json::from_str(json).unwrap();
        assert_eq!(callback.code, "abc123");
        assert_eq!(callback.state, "xyz789");
        assert!(callback.error.is_none());
    }

    #[test]
    fn test_oauth_callback_with_error() {
        let json = r#"{"code": "", "state": "", "error": "access_denied", "error_description": "User denied access"}"#;
        let callback: OAuthCallback = serde_json::from_str(json).unwrap();
        assert_eq!(callback.error, Some("access_denied".to_string()));
        assert_eq!(
            callback.error_description,
            Some("User denied access".to_string())
        );
    }
}

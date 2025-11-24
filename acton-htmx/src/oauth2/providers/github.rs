//! GitHub OAuth2 provider implementation
//!
//! This module implements OAuth2 authentication with GitHub using their OAuth2 API.

use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, EndpointNotSet, EndpointSet, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};

use crate::oauth2::http::async_http_client;
use crate::oauth2::types::{OAuthError, OAuthToken, OAuthUserInfo, ProviderConfig};

// BasicClient with auth and token endpoints set
type ConfiguredClient = BasicClient<
    EndpointSet,          // HasAuthUrl
    EndpointNotSet,       // HasDeviceAuthUrl
    EndpointNotSet,       // HasIntrospectionUrl
    EndpointNotSet,       // HasRevocationUrl
    EndpointSet,          // HasTokenUrl
>;

/// GitHub OAuth2 provider
pub struct GitHubProvider {
    client: ConfiguredClient,
}

impl GitHubProvider {
    /// Create a new GitHub OAuth2 provider
    ///
    /// # Errors
    ///
    /// Returns error if the configuration is invalid
    pub fn new(config: &ProviderConfig) -> Result<Self, OAuthError> {
        // oauth2 5.0 API: BasicClient::new() only takes ClientId
        let client = BasicClient::new(ClientId::new(config.client_id.clone()))
            .set_client_secret(ClientSecret::new(config.client_secret.clone()))
            .set_auth_uri(
                AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
                    .map_err(|e| OAuthError::Generic(format!("Invalid auth URL: {e}")))?,
            )
            .set_token_uri(
                TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
                    .map_err(|e| OAuthError::Generic(format!("Invalid token URL: {e}")))?,
            )
            .set_redirect_uri(
                RedirectUrl::new(config.redirect_uri.clone())
                    .map_err(|e| OAuthError::Generic(format!("Invalid redirect URI: {e}")))?,
            );

        Ok(Self { client })
    }

    /// Generate authorization URL and CSRF state
    ///
    /// Returns tuple of (authorization_url, csrf_state, pkce_verifier)
    #[must_use]
    pub fn authorization_url(&self) -> (String, String, String) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let (auth_url, csrf_state) = self
            .client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("read:user".to_string()))
            .add_scope(Scope::new("user:email".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        (
            auth_url.to_string(),
            csrf_state.secret().clone(),
            pkce_verifier.secret().clone(),
        )
    }

    /// Exchange authorization code for access token
    ///
    /// # Errors
    ///
    /// Returns error if the token exchange fails
    pub async fn exchange_code(
        &self,
        code: &str,
        pkce_verifier: &str,
    ) -> Result<OAuthToken, OAuthError> {
        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier.to_string()))
            .request_async(&async_http_client)
            .await
            .map_err(|e| OAuthError::TokenExchangeFailed(e.to_string()))?;

        Ok(OAuthToken {
            access_token: token_response.access_token().secret().clone(),
            refresh_token: token_response
                .refresh_token()
                .map(|t| t.secret().clone()),
            token_type: "Bearer".to_string(),
            expires_at: token_response.expires_in().map(|duration| {
                std::time::SystemTime::now() + std::time::Duration::from_secs(duration.as_secs())
            }),
            scopes: token_response
                .scopes()
                .map(|scopes| scopes.iter().map(|s| s.to_string()).collect()),
        })
    }

    /// Fetch user information using access token
    ///
    /// # Errors
    ///
    /// Returns error if the user info request fails
    pub async fn fetch_user_info(&self, access_token: &str) -> Result<OAuthUserInfo, OAuthError> {
        let client = reqwest::Client::new();

        // Fetch user profile
        let user_response = client
            .get("https://api.github.com/user")
            .bearer_auth(access_token)
            .header("User-Agent", "acton-htmx")
            .send()
            .await
            .map_err(|e| OAuthError::UserInfoFailed(e.to_string()))?;

        if !user_response.status().is_success() {
            return Err(OAuthError::UserInfoFailed(format!(
                "HTTP {}",
                user_response.status()
            )));
        }

        let github_user: GitHubUser = user_response
            .json()
            .await
            .map_err(|e| OAuthError::UserInfoFailed(format!("Failed to parse user JSON: {e}")))?;

        // Fetch user emails (to get primary verified email)
        let emails_response = client
            .get("https://api.github.com/user/emails")
            .bearer_auth(access_token)
            .header("User-Agent", "acton-htmx")
            .send()
            .await
            .map_err(|e| OAuthError::UserInfoFailed(e.to_string()))?;

        let emails: Vec<GitHubEmail> = if emails_response.status().is_success() {
            emails_response
                .json()
                .await
                .map_err(|e| OAuthError::UserInfoFailed(format!("Failed to parse emails JSON: {e}")))?
        } else {
            vec![]
        };

        // Find primary verified email
        let primary_email = emails
            .iter()
            .find(|e| e.primary && e.verified)
            .or_else(|| emails.iter().find(|e| e.verified))
            .or_else(|| emails.first());

        let email = primary_email.map_or_else(
            || format!("{}@users.noreply.github.com", github_user.id),
            |e| e.email.clone(),
        );

        let email_verified = primary_email.is_some_and(|e| e.verified);

        Ok(OAuthUserInfo {
            provider_user_id: github_user.id.to_string(),
            email,
            name: github_user.name.or(Some(github_user.login)),
            avatar_url: Some(github_user.avatar_url),
            email_verified,
        })
    }
}

/// GitHub user response
#[derive(Debug, Deserialize, Serialize)]
struct GitHubUser {
    id: i64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: String,
}

/// GitHub email response
#[derive(Debug, Deserialize, Serialize)]
struct GitHubEmail {
    email: String,
    verified: bool,
    primary: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_provider_creation() {
        let config = ProviderConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_uri: "http://localhost:3000/auth/github/callback".to_string(),
            scopes: vec!["read:user".to_string(), "user:email".to_string()],
            auth_url: None,
            token_url: None,
            userinfo_url: None,
        };

        let provider = GitHubProvider::new(&config).unwrap();
        let (auth_url, csrf_state, pkce_verifier) = provider.authorization_url();

        assert!(auth_url.starts_with("https://github.com/login/oauth/authorize"));
        assert!(auth_url.contains("client_id=test-client-id"));
        assert!(auth_url.contains("redirect_uri"));
        assert!(auth_url.contains("scope=read%3Auser"));
        assert!(!csrf_state.is_empty());
        assert!(!pkce_verifier.is_empty());
    }
}

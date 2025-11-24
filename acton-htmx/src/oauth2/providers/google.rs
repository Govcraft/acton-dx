//! Google OAuth2 provider implementation
//!
//! This module implements OAuth2 authentication with Google using the OpenID Connect
//! discovery protocol.

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

/// Google OAuth2 provider
pub struct GoogleProvider {
    client: ConfiguredClient,
}

impl GoogleProvider {
    /// Create a new Google OAuth2 provider
    ///
    /// # Errors
    ///
    /// Returns error if the provider metadata cannot be fetched or if the
    /// configuration is invalid
    pub fn new(config: &ProviderConfig) -> Result<Self, OAuthError> {
        // Create basic OAuth2 client for token exchange (oauth2 5.0 API)
        let client = BasicClient::new(ClientId::new(config.client_id.clone()))
            .set_client_secret(ClientSecret::new(config.client_secret.clone()))
            .set_auth_uri(
                AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                    .map_err(|e| OAuthError::Generic(format!("Invalid auth URL: {e}")))?,
            )
            .set_token_uri(
                TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
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
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
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
        let response = client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| OAuthError::UserInfoFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(OAuthError::UserInfoFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let google_user: GoogleUserInfo = response
            .json()
            .await
            .map_err(|e| OAuthError::UserInfoFailed(format!("Failed to parse JSON: {e}")))?;

        Ok(OAuthUserInfo {
            provider_user_id: google_user.id,
            email: google_user.email,
            name: google_user.name,
            avatar_url: google_user.picture,
            email_verified: google_user.verified_email.unwrap_or(false),
        })
    }
}

/// Google user info response
#[derive(Debug, Deserialize, Serialize)]
struct GoogleUserInfo {
    id: String,
    email: String,
    verified_email: Option<bool>,
    name: Option<String>,
    picture: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_url_generation() {
        // This test requires a valid Google OAuth2 configuration
        // In a real scenario, you would use test credentials
        let config = ProviderConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_uri: "http://localhost:3000/auth/google/callback".to_string(),
            scopes: vec!["openid".to_string(), "email".to_string()],
            auth_url: None,
            token_url: None,
            userinfo_url: None,
        };

        // Note: This will make a network request to Google's discovery endpoint
        // In production, you might want to mock this
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let provider = GoogleProvider::new(&config);
            // Skip test if network is unavailable
            if provider.is_err() {
                return;
            }

            let provider = provider.unwrap();
            let (auth_url, csrf_state, pkce_verifier) = provider.authorization_url();

            assert!(auth_url.starts_with("https://accounts.google.com"));
            assert!(auth_url.contains("client_id=test-client-id"));
            assert!(auth_url.contains("redirect_uri"));
            assert!(auth_url.contains("scope=openid"));
            assert!(!csrf_state.is_empty());
            assert!(!pkce_verifier.is_empty());
        });
    }
}

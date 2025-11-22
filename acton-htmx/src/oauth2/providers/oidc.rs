//! Generic OpenID Connect provider implementation
//!
//! This module implements a generic OIDC provider that can work with any
//! OpenID Connect compliant identity provider (Okta, Auth0, Keycloak, etc.).

use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, EndpointNotSet, EndpointSet, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use openidconnect::{
    core::CoreProviderMetadata,
    IssuerUrl,
};
use serde::{Deserialize, Serialize};

use crate::oauth2::types::{OAuthError, OAuthToken, OAuthUserInfo, ProviderConfig};

// BasicClient with auth and token endpoints set
type ConfiguredClient = BasicClient<
    EndpointSet,          // HasAuthUrl
    EndpointNotSet,       // HasDeviceAuthUrl
    EndpointNotSet,       // HasIntrospectionUrl
    EndpointNotSet,       // HasRevocationUrl
    EndpointSet,          // HasTokenUrl
>;

/// Async HTTP client for OAuth2 requests
async fn async_http_client(
    request: oauth2::HttpRequest,
) -> Result<oauth2::HttpResponse, reqwest::Error> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let method = request.method().clone();
    let url = request.uri().to_string();
    let headers = request.headers().clone();
    let body = request.into_body();

    let mut request_builder = client
        .request(method, &url)
        .body(body);

    for (name, value) in &headers {
        request_builder = request_builder.header(name.as_str(), value.as_bytes());
    }

    let response = request_builder
        .send()
        .await?;

    let status_code = response.status();
    let headers = response.headers().to_owned();
    let body = response
        .bytes()
        .await?
        .to_vec();

    let mut builder = http::Response::builder().status(status_code);
    for (name, value) in &headers {
        builder = builder.header(name, value);
    }
    // This should never fail as we're building with valid components
    Ok(builder.body(body).expect("Failed to build HTTP response"))
}

/// Generic OpenID Connect provider
pub struct OidcProvider {
    client: ConfiguredClient,
    userinfo_url: String,
}

impl OidcProvider {
    /// Create a new generic OIDC provider
    ///
    /// # Errors
    ///
    /// Returns error if the configuration is invalid or if discovery fails
    ///
    /// # Panics
    ///
    /// This function should not panic as all unwrap() calls are guarded by is_some() checks
    pub async fn new(config: &ProviderConfig) -> Result<Self, OAuthError> {
        // For generic OIDC, we require either:
        // 1. Manual configuration (all three URLs: auth_url, token_url, userinfo_url)
        // 2. Discovery via issuer URL (only auth_url provided)

        let (client, userinfo_url) = if config.auth_url.is_some()
            && config.token_url.is_some()
            && config.userinfo_url.is_some()
        {
            // Manual configuration - all URLs provided
            // SAFETY: These unwraps are safe because we just checked is_some() above
            let auth_url = config.auth_url.as_ref().unwrap();
            let token_url = config.token_url.as_ref().unwrap();
            let userinfo_url = config.userinfo_url.as_ref().unwrap();

            Self::manual_config(config, auth_url, token_url, userinfo_url)?
        } else if let Some(issuer_url) = &config.auth_url {
            // Discovery - only issuer URL provided
            Self::discover(config, issuer_url).await?
        } else {
            return Err(OAuthError::Generic(
                "Either provide all URLs (auth_url, token_url, userinfo_url) for manual config, or just auth_url for discovery".to_string(),
            ));
        };

        Ok(Self {
            client,
            userinfo_url,
        })
    }

    /// Discover OIDC configuration from issuer
    async fn discover(
        config: &ProviderConfig,
        issuer: &str,
    ) -> Result<(ConfiguredClient, String), OAuthError> {
        let issuer_url = IssuerUrl::new(issuer.to_string())
            .map_err(|e| OAuthError::Generic(format!("Invalid issuer URL: {e}")))?;

        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &async_http_client)
            .await
            .map_err(|e| OAuthError::Generic(format!("Failed to discover provider: {e}")))?;

        let userinfo_url = provider_metadata
            .userinfo_endpoint()
            .ok_or_else(|| OAuthError::Generic("Provider has no userinfo endpoint".to_string()))?
            .to_string();

        let auth_url = provider_metadata
            .authorization_endpoint()
            .to_string();
        let token_url = provider_metadata
            .token_endpoint()
            .ok_or_else(|| OAuthError::Generic("Provider has no token endpoint".to_string()))?
            .to_string();

        // oauth2 5.0 API: BasicClient::new() only takes ClientId
        let client = BasicClient::new(ClientId::new(config.client_id.clone()))
            .set_client_secret(ClientSecret::new(config.client_secret.clone()))
            .set_auth_uri(
                AuthUrl::new(auth_url)
                    .map_err(|e| OAuthError::Generic(format!("Invalid auth URL: {e}")))?,
            )
            .set_token_uri(
                TokenUrl::new(token_url)
                    .map_err(|e| OAuthError::Generic(format!("Invalid token URL: {e}")))?,
            )
            .set_redirect_uri(
                RedirectUrl::new(config.redirect_uri.clone())
                    .map_err(|e| OAuthError::Generic(format!("Invalid redirect URI: {e}")))?,
            );

        Ok((client, userinfo_url))
    }

    /// Manual OIDC configuration (no discovery)
    fn manual_config(
        config: &ProviderConfig,
        auth_url: &str,
        token_url: &str,
        userinfo_url: &str,
    ) -> Result<(ConfiguredClient, String), OAuthError> {
        // oauth2 5.0 API: BasicClient::new() only takes ClientId
        let client = BasicClient::new(ClientId::new(config.client_id.clone()))
            .set_client_secret(ClientSecret::new(config.client_secret.clone()))
            .set_auth_uri(
                AuthUrl::new(auth_url.to_string())
                    .map_err(|e| OAuthError::Generic(format!("Invalid auth URL: {e}")))?,
            )
            .set_token_uri(
                TokenUrl::new(token_url.to_string())
                    .map_err(|e| OAuthError::Generic(format!("Invalid token URL: {e}")))?,
            )
            .set_redirect_uri(
                RedirectUrl::new(config.redirect_uri.clone())
                    .map_err(|e| OAuthError::Generic(format!("Invalid redirect URI: {e}")))?,
            );

        Ok((client, userinfo_url.to_string()))
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
            .get(&self.userinfo_url)
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

        let oidc_user: OidcUserInfo = response
            .json()
            .await
            .map_err(|e| OAuthError::UserInfoFailed(format!("Failed to parse JSON: {e}")))?;

        Ok(OAuthUserInfo {
            provider_user_id: oidc_user.sub,
            email: oidc_user.email,
            name: oidc_user.name,
            avatar_url: oidc_user.picture,
            email_verified: oidc_user.email_verified.unwrap_or(false),
        })
    }
}

/// OIDC user info response
#[derive(Debug, Deserialize, Serialize)]
struct OidcUserInfo {
    sub: String,
    email: String,
    email_verified: Option<bool>,
    name: Option<String>,
    picture: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oidc_manual_config() {
        let config = ProviderConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-client-secret".to_string(),
            redirect_uri: "http://localhost:3000/auth/oidc/callback".to_string(),
            scopes: vec!["openid".to_string(), "email".to_string()],
            auth_url: Some("https://example.com/oauth/authorize".to_string()),
            token_url: Some("https://example.com/oauth/token".to_string()),
            userinfo_url: Some("https://example.com/oauth/userinfo".to_string()),
        };

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let provider = OidcProvider::new(&config).await.unwrap();
            let (auth_url, csrf_state, pkce_verifier) = provider.authorization_url();

            assert!(auth_url.starts_with("https://example.com/oauth/authorize"));
            assert!(auth_url.contains("client_id=test-client-id"));
            assert!(!csrf_state.is_empty());
            assert!(!pkce_verifier.is_empty());
        });
    }
}

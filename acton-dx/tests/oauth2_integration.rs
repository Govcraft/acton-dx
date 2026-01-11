//! OAuth2 integration tests
//!
//! Tests the OAuth2 authentication infrastructure including:
//! - State generation and validation (CSRF protection)
//! - Provider configuration
//! - State token lifecycle
//!
//! Note: Full end-to-end OAuth2 tests with token exchange and user creation
//! would require mocking external OAuth providers and a test database.

use acton_dx::oauth2::{
    CleanupExpired, GenerateState, OAuth2Agent, OAuthConfig, OAuthProvider, ProviderConfig,
    RemoveState, ValidateState,
};
use acton_reactive::prelude::{ActonApp, ActorHandleInterface};

/// Helper to create a test OAuth2 configuration
fn test_oauth_config() -> OAuthConfig {
    OAuthConfig {
        google: Some(ProviderConfig {
            client_id: "test-google-client-id".to_string(),
            client_secret: "test-google-client-secret".to_string(),
            redirect_uri: "http://localhost:3000/auth/google/callback".to_string(),
            scopes: vec!["openid".to_string(), "email".to_string(), "profile".to_string()],
            auth_url: Some("https://accounts.google.com/o/oauth2/v2/auth".to_string()),
            token_url: Some("https://oauth2.googleapis.com/token".to_string()),
            userinfo_url: Some("https://openidconnect.googleapis.com/v1/userinfo".to_string()),
        }),
        github: Some(ProviderConfig {
            client_id: "test-github-client-id".to_string(),
            client_secret: "test-github-client-secret".to_string(),
            redirect_uri: "http://localhost:3000/auth/github/callback".to_string(),
            scopes: vec!["read:user".to_string(), "user:email".to_string()],
            auth_url: Some("https://github.com/login/oauth/authorize".to_string()),
            token_url: Some("https://github.com/login/oauth/access_token".to_string()),
            userinfo_url: Some("https://api.github.com/user".to_string()),
        }),
        oidc: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth_state_token_generation() {
    let mut runtime = ActonApp::launch_async().await;
    let oauth2_agent = OAuth2Agent::spawn(&mut runtime)
        .await
        .expect("Failed to spawn OAuth2 agent");

    // Generate state token
    let (generate_msg, rx) = GenerateState::new(OAuthProvider::Google);
    oauth2_agent.send(generate_msg).await;
    let oauth_state = rx.await.expect("Failed to receive state");

    // Verify state token properties
    assert_eq!(oauth_state.provider, OAuthProvider::Google);
    assert_eq!(oauth_state.token.len(), 64); // 32 bytes as hex = 64 chars
    assert!(oauth_state.expires_at > std::time::SystemTime::now());

    // Cleanup
    runtime.shutdown_all().await.ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth_state_token_validation() {
    let mut runtime = ActonApp::launch_async().await;
    let oauth2_agent = OAuth2Agent::spawn(&mut runtime)
        .await
        .expect("Failed to spawn OAuth2 agent");

    // Generate state token
    let (generate_msg, rx) = GenerateState::new(OAuthProvider::GitHub);
    oauth2_agent.send(generate_msg).await;
    let oauth_state = rx.await.expect("Failed to receive state");

    // Validate the token
    let (validate_msg, validate_rx) = ValidateState::new(oauth_state.token.clone());
    oauth2_agent.send(validate_msg).await;
    let validated = validate_rx
        .await
        .expect("Failed to receive validation result");

    // Should be valid
    assert!(validated.is_some());
    let validated_state = validated.unwrap();
    assert_eq!(validated_state.provider, OAuthProvider::GitHub);
    assert_eq!(validated_state.token, oauth_state.token);

    // Cleanup
    runtime.shutdown_all().await.ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth_state_token_validation_fails_for_invalid_token() {
    let mut runtime = ActonApp::launch_async().await;
    let oauth2_agent = OAuth2Agent::spawn(&mut runtime)
        .await
        .expect("Failed to spawn OAuth2 agent");

    // Try to validate a non-existent token
    let (validate_msg, validate_rx) = ValidateState::new("invalid-token-12345".to_string());
    oauth2_agent.send(validate_msg).await;
    let validated = validate_rx
        .await
        .expect("Failed to receive validation result");

    // Should be invalid
    assert!(validated.is_none());

    // Cleanup
    runtime.shutdown_all().await.ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth_state_token_one_time_use() {
    let mut runtime = ActonApp::launch_async().await;
    let oauth2_agent = OAuth2Agent::spawn(&mut runtime)
        .await
        .expect("Failed to spawn OAuth2 agent");

    // Generate state token
    let (generate_msg, rx) = GenerateState::new(OAuthProvider::Google);
    oauth2_agent.send(generate_msg).await;
    let oauth_state = rx.await.expect("Failed to receive state");

    // Validate once (should succeed)
    let (validate_msg1, validate_rx1) = ValidateState::new(oauth_state.token.clone());
    oauth2_agent.send(validate_msg1).await;
    let validated1 = validate_rx1
        .await
        .expect("Failed to receive validation result");
    assert!(validated1.is_some());

    // Remove the token (simulating successful OAuth callback)
    oauth2_agent
        .send(RemoveState {
            token: oauth_state.token.clone(),
        })
        .await;

    // Try to validate again (should fail - token removed)
    let (validate_msg2, validate_rx2) = ValidateState::new(oauth_state.token.clone());
    oauth2_agent.send(validate_msg2).await;
    let validated2 = validate_rx2
        .await
        .expect("Failed to receive validation result");
    assert!(validated2.is_none());

    // Cleanup
    runtime.shutdown_all().await.ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth_state_cleanup_expired_tokens() {
    let mut runtime = ActonApp::launch_async().await;
    let oauth2_agent = OAuth2Agent::spawn(&mut runtime)
        .await
        .expect("Failed to spawn OAuth2 agent");

    // Generate a state token
    let (generate_msg, rx) = GenerateState::new(OAuthProvider::Google);
    oauth2_agent.send(generate_msg).await;
    let oauth_state = rx.await.expect("Failed to receive state");

    // Verify it's valid
    let (validate_msg1, validate_rx1) = ValidateState::new(oauth_state.token.clone());
    oauth2_agent.send(validate_msg1).await;
    let validated1 = validate_rx1
        .await
        .expect("Failed to receive validation result");
    assert!(validated1.is_some());

    // Trigger cleanup (in a real scenario, this would be called periodically)
    oauth2_agent.send(CleanupExpired).await;

    // Token should still be valid (not expired yet)
    let (validate_msg2, validate_rx2) = ValidateState::new(oauth_state.token.clone());
    oauth2_agent.send(validate_msg2).await;
    let validated2 = validate_rx2
        .await
        .expect("Failed to receive validation result");
    assert!(validated2.is_some());

    // Cleanup
    runtime.shutdown_all().await.ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth_provider_config_from_config() {
    let oauth_config = test_oauth_config();

    // Test Google provider config
    let google_config = oauth_config
        .get_provider(OAuthProvider::Google)
        .expect("Google provider should be configured");
    assert_eq!(google_config.client_id, "test-google-client-id");
    assert_eq!(
        google_config.redirect_uri,
        "http://localhost:3000/auth/google/callback"
    );
    assert_eq!(google_config.scopes.len(), 3);

    // Test GitHub provider config
    let github_config = oauth_config
        .get_provider(OAuthProvider::GitHub)
        .expect("GitHub provider should be configured");
    assert_eq!(github_config.client_id, "test-github-client-id");
    assert_eq!(
        github_config.redirect_uri,
        "http://localhost:3000/auth/github/callback"
    );
    assert_eq!(github_config.scopes.len(), 2);

    // Test unconfigured provider
    let oidc_result = oauth_config.get_provider(OAuthProvider::Oidc);
    assert!(oidc_result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth_multiple_state_tokens_can_coexist() {
    let mut runtime = ActonApp::launch_async().await;
    let oauth2_agent = OAuth2Agent::spawn(&mut runtime)
        .await
        .expect("Failed to spawn OAuth2 agent");

    // Generate multiple state tokens for different providers
    let (google_msg, google_rx) = GenerateState::new(OAuthProvider::Google);
    oauth2_agent.send(google_msg).await;
    let google_state = google_rx.await.expect("Failed to receive Google state");

    let (github_msg, github_rx) = GenerateState::new(OAuthProvider::GitHub);
    oauth2_agent.send(github_msg).await;
    let github_state = github_rx.await.expect("Failed to receive GitHub state");

    // Both should be valid and independent
    let (validate_google, google_val_rx) = ValidateState::new(google_state.token.clone());
    oauth2_agent.send(validate_google).await;
    assert!(google_val_rx.await.unwrap().is_some());

    let (validate_github, github_val_rx) = ValidateState::new(github_state.token.clone());
    oauth2_agent.send(validate_github).await;
    assert!(github_val_rx.await.unwrap().is_some());

    // Cleanup
    runtime.shutdown_all().await.ok();
}

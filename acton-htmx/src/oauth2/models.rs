//! OAuth2 account database models
//!
//! This module provides the `OAuthAccount` model for managing OAuth2 provider
//! accounts linked to users.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

use super::types::{OAuthProvider, OAuthUserInfo};

/// OAuth2 account linked to a user
///
/// This represents a connection between a local user account and an OAuth2
/// provider account (Google, GitHub, or generic OIDC).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OAuthAccount {
    /// Primary key
    pub id: i64,
    /// Local user ID
    pub user_id: i64,
    /// OAuth2 provider
    #[sqlx(try_from = "String")]
    pub provider: OAuthProvider,
    /// Provider-specific user ID
    pub provider_user_id: String,
    /// Email from OAuth provider
    pub email: String,
    /// Display name from OAuth provider
    pub name: Option<String>,
    /// Avatar URL from OAuth provider
    pub avatar_url: Option<String>,
    /// When the account was linked
    pub created_at: DateTime<Utc>,
    /// When the account was last updated
    pub updated_at: DateTime<Utc>,
}

impl OAuthAccount {
    /// Find an OAuth account by provider and provider user ID
    ///
    /// # Errors
    ///
    /// Returns error if the database query fails
    pub async fn find_by_provider(
        pool: &PgPool,
        provider: OAuthProvider,
        provider_user_id: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r"
            SELECT id, user_id, provider, provider_user_id, email, name, avatar_url,
                   created_at, updated_at
            FROM oauth_accounts
            WHERE provider = $1 AND provider_user_id = $2
            ",
        )
        .bind(provider.as_str())
        .bind(provider_user_id)
        .fetch_optional(pool)
        .await
    }

    /// Find all OAuth accounts for a user
    ///
    /// # Errors
    ///
    /// Returns error if the database query fails
    pub async fn find_by_user_id(pool: &PgPool, user_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r"
            SELECT id, user_id, provider, provider_user_id, email, name, avatar_url,
                   created_at, updated_at
            FROM oauth_accounts
            WHERE user_id = $1
            ORDER BY created_at DESC
            ",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    /// Link an OAuth account to a user
    ///
    /// # Errors
    ///
    /// Returns error if the database query fails or if the OAuth account
    /// is already linked to a different user
    pub async fn link_account(
        pool: &PgPool,
        user_id: i64,
        provider: OAuthProvider,
        user_info: &OAuthUserInfo,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r"
            INSERT INTO oauth_accounts (user_id, provider, provider_user_id, email, name, avatar_url)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (provider, provider_user_id)
            DO UPDATE SET
                user_id = EXCLUDED.user_id,
                email = EXCLUDED.email,
                name = EXCLUDED.name,
                avatar_url = EXCLUDED.avatar_url,
                updated_at = NOW()
            RETURNING id, user_id, provider, provider_user_id, email, name, avatar_url,
                      created_at, updated_at
            ",
        )
        .bind(user_id)
        .bind(provider.as_str())
        .bind(&user_info.provider_user_id)
        .bind(&user_info.email)
        .bind(&user_info.name)
        .bind(&user_info.avatar_url)
        .fetch_one(pool)
        .await
    }

    /// Unlink an OAuth account
    ///
    /// # Errors
    ///
    /// Returns error if the database query fails
    pub async fn unlink_account(
        pool: &PgPool,
        user_id: i64,
        provider: OAuthProvider,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r"
            DELETE FROM oauth_accounts
            WHERE user_id = $1 AND provider = $2
            ",
        )
        .bind(user_id)
        .bind(provider.as_str())
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update OAuth account information
    ///
    /// # Errors
    ///
    /// Returns error if the database query fails
    pub async fn update_info(
        &mut self,
        pool: &PgPool,
        user_info: &OAuthUserInfo,
    ) -> Result<(), sqlx::Error> {
        let updated = sqlx::query_as::<_, Self>(
            r"
            UPDATE oauth_accounts
            SET email = $1, name = $2, avatar_url = $3, updated_at = NOW()
            WHERE id = $4
            RETURNING id, user_id, provider, provider_user_id, email, name, avatar_url,
                      created_at, updated_at
            ",
        )
        .bind(&user_info.email)
        .bind(&user_info.name)
        .bind(&user_info.avatar_url)
        .bind(self.id)
        .fetch_one(pool)
        .await?;

        *self = updated;
        Ok(())
    }

    /// Check if a user has any OAuth accounts linked
    ///
    /// # Errors
    ///
    /// Returns error if the database query fails
    pub async fn user_has_oauth_accounts(
        pool: &PgPool,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let count: (i64,) = sqlx::query_as(
            r"
            SELECT COUNT(*) FROM oauth_accounts WHERE user_id = $1
            ",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(count.0 > 0)
    }
}

// SQLx type conversion for OAuthProvider
impl TryFrom<String> for OAuthProvider {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse().map_err(|e: super::types::OAuthError| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_account_serialization() {
        let account = OAuthAccount {
            id: 1,
            user_id: 100,
            provider: OAuthProvider::Google,
            provider_user_id: "123456".to_string(),
            email: "test@gmail.com".to_string(),
            name: Some("Test User".to_string()),
            avatar_url: Some("https://example.com/avatar.jpg".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Test serialization
        let json = serde_json::to_string(&account).unwrap();
        assert!(json.contains("123456"));
        assert!(json.contains("test@gmail.com"));

        // Test deserialization
        let deserialized: OAuthAccount = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.user_id, 100);
        assert_eq!(deserialized.provider, OAuthProvider::Google);
        assert_eq!(deserialized.provider_user_id, "123456");
        assert_eq!(deserialized.email, "test@gmail.com");
    }

    #[test]
    fn test_oauth_provider_try_from_string() {
        // Test valid conversions
        assert_eq!(
            OAuthProvider::try_from("google".to_string()).unwrap(),
            OAuthProvider::Google
        );
        assert_eq!(
            OAuthProvider::try_from("github".to_string()).unwrap(),
            OAuthProvider::GitHub
        );
        assert_eq!(
            OAuthProvider::try_from("oidc".to_string()).unwrap(),
            OAuthProvider::Oidc
        );

        // Test invalid conversion
        assert!(OAuthProvider::try_from("invalid".to_string()).is_err());
    }

    #[test]
    fn test_oauth_account_debug() {
        let account = OAuthAccount {
            id: 1,
            user_id: 100,
            provider: OAuthProvider::GitHub,
            provider_user_id: "gh123".to_string(),
            email: "test@github.com".to_string(),
            name: None,
            avatar_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let debug_str = format!("{account:?}");
        assert!(debug_str.contains("OAuthAccount"));
        assert!(debug_str.contains("GitHub"));
        assert!(debug_str.contains("gh123"));
    }

    #[test]
    fn test_oauth_account_clone() {
        let account = OAuthAccount {
            id: 1,
            user_id: 100,
            provider: OAuthProvider::Google,
            provider_user_id: "123456".to_string(),
            email: "test@gmail.com".to_string(),
            name: Some("Test User".to_string()),
            avatar_url: Some("https://example.com/avatar.jpg".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let cloned = account.clone();
        assert_eq!(cloned.id, account.id);
        assert_eq!(cloned.user_id, account.user_id);
        assert_eq!(cloned.provider, account.provider);
        assert_eq!(cloned.provider_user_id, account.provider_user_id);
        assert_eq!(cloned.email, account.email);
    }
}

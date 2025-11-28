//! Auth service client for sessions, passwords, CSRF, and users.

use super::error::ClientError;
use acton_dx_proto::auth::v1::{
    csrf_service_client::CsrfServiceClient, password_service_client::PasswordServiceClient,
    session_service_client::SessionServiceClient, user_service_client::UserServiceClient,
    AddFlashMessageRequest, CreateSessionRequest, CreateUserRequest, DeleteUserRequest,
    DestroySessionRequest, FlashMessage, GenerateTokenRequest, GetFlashMessagesRequest,
    GetUserByEmailRequest, GetUserRequest, HashPasswordRequest, Session, UpdateSessionRequest,
    UpdateUserRequest, User, ValidateSessionRequest, ValidateTokenRequest, VerifyPasswordRequest,
};
use std::collections::HashMap;
use tonic::transport::Channel;

/// Client for the auth service.
///
/// Provides access to session management, password hashing/verification,
/// CSRF token handling, and user CRUD operations.
#[derive(Debug, Clone)]
pub struct AuthClient {
    sessions: SessionServiceClient<Channel>,
    passwords: PasswordServiceClient<Channel>,
    csrf: CsrfServiceClient<Channel>,
    users: UserServiceClient<Channel>,
}

impl AuthClient {
    /// Connect to the auth service.
    ///
    /// # Errors
    ///
    /// Returns error if connection fails.
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, ClientError> {
        let endpoint = endpoint.into();
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| ClientError::ConnectionFailed(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            sessions: SessionServiceClient::new(channel.clone()),
            passwords: PasswordServiceClient::new(channel.clone()),
            csrf: CsrfServiceClient::new(channel.clone()),
            users: UserServiceClient::new(channel),
        })
    }

    // ==================== Session Operations ====================

    /// Create a new session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn create_session(
        &mut self,
        user_id: Option<i64>,
        ttl_seconds: i64,
        initial_data: HashMap<String, String>,
    ) -> Result<Session, ClientError> {
        let response = self
            .sessions
            .create_session(CreateSessionRequest {
                user_id,
                ttl_seconds,
                initial_data,
            })
            .await?;

        response
            .into_inner()
            .session
            .ok_or_else(|| ClientError::ResponseError("No session in response".to_string()))
    }

    /// Validate an existing session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn validate_session(
        &mut self,
        session_id: &str,
    ) -> Result<Option<Session>, ClientError> {
        let response = self
            .sessions
            .validate_session(ValidateSessionRequest {
                session_id: session_id.to_string(),
            })
            .await?;

        let inner = response.into_inner();
        if inner.valid {
            Ok(inner.session)
        } else {
            Ok(None)
        }
    }

    /// Update session data.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn update_session(
        &mut self,
        session_id: &str,
        data: HashMap<String, String>,
        user_id: Option<i64>,
    ) -> Result<Option<Session>, ClientError> {
        let response = self
            .sessions
            .update_session(UpdateSessionRequest {
                session_id: session_id.to_string(),
                data,
                user_id,
            })
            .await?;

        let inner = response.into_inner();
        if inner.success {
            Ok(inner.session)
        } else {
            Ok(None)
        }
    }

    /// Destroy a session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn destroy_session(&mut self, session_id: &str) -> Result<bool, ClientError> {
        let response = self
            .sessions
            .destroy_session(DestroySessionRequest {
                session_id: session_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().success)
    }

    /// Add a flash message to a session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn add_flash_message(
        &mut self,
        session_id: &str,
        level: &str,
        message: &str,
    ) -> Result<bool, ClientError> {
        let response = self
            .sessions
            .add_flash_message(AddFlashMessageRequest {
                session_id: session_id.to_string(),
                flash: Some(FlashMessage {
                    level: level.to_string(),
                    message: message.to_string(),
                }),
            })
            .await?;

        Ok(response.into_inner().success)
    }

    /// Get and clear flash messages for a session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn get_and_clear_flash_messages(
        &mut self,
        session_id: &str,
    ) -> Result<Vec<FlashMessage>, ClientError> {
        let response = self
            .sessions
            .get_and_clear_flash_messages(GetFlashMessagesRequest {
                session_id: session_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().messages)
    }

    // ==================== Password Operations ====================

    /// Hash a password using Argon2.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn hash_password(&mut self, password: &str) -> Result<String, ClientError> {
        let response = self
            .passwords
            .hash_password(HashPasswordRequest {
                password: password.to_string(),
            })
            .await?;

        Ok(response.into_inner().hash)
    }

    /// Verify a password against a hash.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn verify_password(
        &mut self,
        password: &str,
        hash: &str,
    ) -> Result<bool, ClientError> {
        let response = self
            .passwords
            .verify_password(VerifyPasswordRequest {
                password: password.to_string(),
                hash: hash.to_string(),
            })
            .await?;

        Ok(response.into_inner().valid)
    }

    // ==================== CSRF Operations ====================

    /// Generate a CSRF token for a session.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn generate_csrf_token(&mut self, session_id: &str) -> Result<String, ClientError> {
        let response = self
            .csrf
            .generate_token(GenerateTokenRequest {
                session_id: session_id.to_string(),
            })
            .await?;

        Ok(response.into_inner().token)
    }

    /// Validate a CSRF token.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn validate_csrf_token(
        &mut self,
        session_id: &str,
        token: &str,
    ) -> Result<bool, ClientError> {
        let response = self
            .csrf
            .validate_token(ValidateTokenRequest {
                session_id: session_id.to_string(),
                token: token.to_string(),
            })
            .await?;

        Ok(response.into_inner().valid)
    }

    // ==================== User Operations ====================

    /// Create a new user.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn create_user(
        &mut self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<Option<User>, ClientError> {
        let response = self
            .users
            .create_user(CreateUserRequest {
                email: email.to_string(),
                name: name.to_string(),
                password: password.to_string(),
            })
            .await?;

        Ok(response.into_inner().user)
    }

    /// Get a user by ID.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn get_user(&mut self, id: i64) -> Result<Option<User>, ClientError> {
        let response = self.users.get_user(GetUserRequest { id }).await?;

        Ok(response.into_inner().user)
    }

    /// Get a user by email.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn get_user_by_email(&mut self, email: &str) -> Result<Option<User>, ClientError> {
        let response = self
            .users
            .get_user_by_email(GetUserByEmailRequest {
                email: email.to_string(),
            })
            .await?;

        Ok(response.into_inner().user)
    }

    /// Update a user.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn update_user(
        &mut self,
        id: i64,
        email: Option<String>,
        name: Option<String>,
        password: Option<String>,
    ) -> Result<Option<User>, ClientError> {
        let response = self
            .users
            .update_user(UpdateUserRequest {
                id,
                email,
                name,
                password,
            })
            .await?;

        Ok(response.into_inner().user)
    }

    /// Delete a user.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn delete_user(&mut self, id: i64) -> Result<bool, ClientError> {
        let response = self.users.delete_user(DeleteUserRequest { id }).await?;

        Ok(response.into_inner().success)
    }
}

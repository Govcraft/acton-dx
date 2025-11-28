//! gRPC CSRF Service implementation.

use acton_dx_proto::auth::v1::{
    csrf_service_server::CsrfService, GenerateTokenRequest, GenerateTokenResponse,
    ValidateTokenRequest, ValidateTokenResponse,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use dashmap::DashMap;
use rand::Rng;
use std::sync::Arc;
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use tonic::{Request, Response, Status};

/// CSRF token data.
#[derive(Debug, Clone)]
struct CsrfToken {
    /// The token value.
    token: String,
    /// When the token expires.
    expires_at: Instant,
}

/// gRPC CSRF Service implementation.
#[derive(Debug, Clone)]
pub struct CsrfServiceImpl {
    /// Token storage: session_id -> CsrfToken.
    tokens: Arc<DashMap<String, CsrfToken>>,
    /// Token TTL.
    token_ttl: Duration,
    /// Token byte length.
    token_bytes: usize,
}

impl CsrfServiceImpl {
    /// Create a new CSRF service with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(DashMap::new()),
            token_ttl: Duration::from_secs(3600),
            token_bytes: 32,
        }
    }

    /// Create a new CSRF service with custom settings.
    #[must_use]
    pub fn with_config(token_ttl_seconds: u64, token_bytes: usize) -> Self {
        Self {
            tokens: Arc::new(DashMap::new()),
            token_ttl: Duration::from_secs(token_ttl_seconds),
            token_bytes,
        }
    }

    /// Generate a random token string.
    fn create_random_token(&self) -> String {
        let mut bytes = vec![0u8; self.token_bytes];
        rand::rng().fill(&mut bytes[..]);
        URL_SAFE_NO_PAD.encode(&bytes)
    }

    /// Cleanup expired tokens.
    #[must_use]
    pub fn cleanup_expired(&self) -> usize {
        let now = Instant::now();
        let before = self.tokens.len();
        self.tokens.retain(|_, token| token.expires_at > now);
        before - self.tokens.len()
    }
}

impl Default for CsrfServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl CsrfService for CsrfServiceImpl {
    async fn generate_token(
        &self,
        request: Request<GenerateTokenRequest>,
    ) -> Result<Response<GenerateTokenResponse>, Status> {
        let req = request.into_inner();

        if req.session_id.is_empty() {
            return Err(Status::invalid_argument("session_id cannot be empty"));
        }

        let token = self.create_random_token();
        let csrf_token = CsrfToken {
            token: token.clone(),
            expires_at: Instant::now() + self.token_ttl,
        };

        self.tokens.insert(req.session_id, csrf_token);

        Ok(Response::new(GenerateTokenResponse { token }))
    }

    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>,
    ) -> Result<Response<ValidateTokenResponse>, Status> {
        let req = request.into_inner();

        if req.session_id.is_empty() {
            return Err(Status::invalid_argument("session_id cannot be empty"));
        }

        if req.token.is_empty() {
            return Err(Status::invalid_argument("token cannot be empty"));
        }

        let valid = self.tokens.get(&req.session_id).is_some_and(|entry| {
            let stored = &entry.token;
            let now = Instant::now();

            // Check expiration first
            if entry.expires_at <= now {
                return false;
            }

            // Constant-time comparison to prevent timing attacks
            let stored_bytes = stored.as_bytes();
            let provided_bytes = req.token.as_bytes();

            // Both must be same length for constant-time comparison
            if stored_bytes.len() != provided_bytes.len() {
                return false;
            }

            stored_bytes.ct_eq(provided_bytes).into()
        });

        Ok(Response::new(ValidateTokenResponse { valid }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_and_validate_token() {
        let service = CsrfServiceImpl::new();

        // Generate a token
        let gen_req = Request::new(GenerateTokenRequest {
            session_id: "session123".to_string(),
        });
        let gen_resp = CsrfService::generate_token(&service, gen_req)
            .await
            .unwrap();
        let token = gen_resp.into_inner().token;

        // Validate correct token
        let val_req = Request::new(ValidateTokenRequest {
            session_id: "session123".to_string(),
            token: token.clone(),
        });
        let val_resp = CsrfService::validate_token(&service, val_req)
            .await
            .unwrap();
        assert!(val_resp.into_inner().valid);

        // Validate incorrect token
        let val_req = Request::new(ValidateTokenRequest {
            session_id: "session123".to_string(),
            token: "wrong-token".to_string(),
        });
        let val_resp = CsrfService::validate_token(&service, val_req)
            .await
            .unwrap();
        assert!(!val_resp.into_inner().valid);
    }

    #[tokio::test]
    async fn test_validate_wrong_session() {
        let service = CsrfServiceImpl::new();

        // Generate a token for one session
        let gen_req = Request::new(GenerateTokenRequest {
            session_id: "session123".to_string(),
        });
        let gen_resp = CsrfService::generate_token(&service, gen_req)
            .await
            .unwrap();
        let token = gen_resp.into_inner().token;

        // Try to validate with different session
        let val_req = Request::new(ValidateTokenRequest {
            session_id: "session456".to_string(),
            token,
        });
        let val_resp = CsrfService::validate_token(&service, val_req)
            .await
            .unwrap();
        assert!(!val_resp.into_inner().valid);
    }

    #[tokio::test]
    async fn test_token_expiration() {
        // Create service with very short TTL
        let service = CsrfServiceImpl::with_config(0, 32);

        // Generate a token
        let gen_req = Request::new(GenerateTokenRequest {
            session_id: "session123".to_string(),
        });
        let gen_resp = CsrfService::generate_token(&service, gen_req)
            .await
            .unwrap();
        let token = gen_resp.into_inner().token;

        // Small delay to ensure expiration
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Token should be expired
        let val_req = Request::new(ValidateTokenRequest {
            session_id: "session123".to_string(),
            token,
        });
        let val_resp = CsrfService::validate_token(&service, val_req)
            .await
            .unwrap();
        assert!(!val_resp.into_inner().valid);
    }

    #[tokio::test]
    async fn test_empty_session_id() {
        let service = CsrfServiceImpl::new();

        let gen_req = Request::new(GenerateTokenRequest {
            session_id: String::new(),
        });
        let result = CsrfService::generate_token(&service, gen_req).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        // Create service with very short TTL
        let service = CsrfServiceImpl::with_config(0, 32);

        // Generate tokens
        for i in 0..5 {
            let gen_req = Request::new(GenerateTokenRequest {
                session_id: format!("session{i}"),
            });
            let _ = CsrfService::generate_token(&service, gen_req)
                .await
                .unwrap();
        }

        assert_eq!(service.tokens.len(), 5);

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Cleanup should remove all
        let removed = service.cleanup_expired();
        assert_eq!(removed, 5);
        assert_eq!(service.tokens.len(), 0);
    }

    #[test]
    fn test_token_length() {
        let service = CsrfServiceImpl::with_config(3600, 64);
        let token = service.create_random_token();
        // Base64 URL-safe encoding: 64 bytes -> 86 characters (no padding)
        assert_eq!(token.len(), 86);
    }
}

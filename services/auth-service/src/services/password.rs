//! gRPC Password Service implementation.

use acton_dx_proto::auth::v1::{
    password_service_server::PasswordService, HashPasswordRequest, HashPasswordResponse,
    VerifyPasswordRequest, VerifyPasswordResponse,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
    Argon2, Params,
};
use tonic::{Request, Response, Status};

/// gRPC Password Service implementation.
#[derive(Debug, Clone)]
pub struct PasswordServiceImpl {
    /// Argon2 hasher configuration.
    argon2: Argon2<'static>,
}

impl PasswordServiceImpl {
    /// Create a new password service with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            argon2: Argon2::default(),
        }
    }

    /// Create a new password service with custom parameters.
    ///
    /// # Panics
    ///
    /// Panics if the parameters are invalid (which should not happen with valid inputs).
    #[must_use]
    pub fn with_params(
        memory_cost: u32,
        time_cost: u32,
        parallelism: u32,
        output_len: Option<usize>,
    ) -> Self {
        let params = Params::new(memory_cost, time_cost, parallelism, output_len)
            .expect("Invalid argon2 parameters");
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        Self { argon2 }
    }
}

impl Default for PasswordServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl PasswordService for PasswordServiceImpl {
    async fn hash_password(
        &self,
        request: Request<HashPasswordRequest>,
    ) -> Result<Response<HashPasswordResponse>, Status> {
        let req = request.into_inner();

        if req.password.is_empty() {
            return Err(Status::invalid_argument("password cannot be empty"));
        }

        // Generate a random salt
        let salt = SaltString::generate(&mut OsRng);

        // Hash the password
        let hash = self
            .argon2
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| Status::internal(format!("Failed to hash password: {e}")))?
            .to_string();

        Ok(Response::new(HashPasswordResponse { hash }))
    }

    async fn verify_password(
        &self,
        request: Request<VerifyPasswordRequest>,
    ) -> Result<Response<VerifyPasswordResponse>, Status> {
        let req = request.into_inner();

        if req.password.is_empty() {
            return Err(Status::invalid_argument("password cannot be empty"));
        }

        if req.hash.is_empty() {
            return Err(Status::invalid_argument("hash cannot be empty"));
        }

        // Parse the stored hash
        let Ok(parsed_hash) = PasswordHash::new(&req.hash) else {
            // Invalid hash format - return false rather than error
            return Ok(Response::new(VerifyPasswordResponse { valid: false }));
        };

        // Verify using constant-time comparison
        let valid = self
            .argon2
            .verify_password(req.password.as_bytes(), &parsed_hash)
            .is_ok();

        Ok(Response::new(VerifyPasswordResponse { valid }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hash_and_verify_password() {
        let service = PasswordServiceImpl::new();

        // Hash a password
        let hash_req = Request::new(HashPasswordRequest {
            password: "mysecretpassword".to_string(),
        });
        let hash_resp = service.hash_password(hash_req).await.unwrap();
        let hash = hash_resp.into_inner().hash;

        // Verify correct password
        let verify_req = Request::new(VerifyPasswordRequest {
            password: "mysecretpassword".to_string(),
            hash: hash.clone(),
        });
        let verify_resp = service.verify_password(verify_req).await.unwrap();
        assert!(verify_resp.into_inner().valid);

        // Verify incorrect password
        let verify_req = Request::new(VerifyPasswordRequest {
            password: "wrongpassword".to_string(),
            hash,
        });
        let verify_resp = service.verify_password(verify_req).await.unwrap();
        assert!(!verify_resp.into_inner().valid);
    }

    #[tokio::test]
    async fn test_hash_empty_password() {
        let service = PasswordServiceImpl::new();

        let hash_req = Request::new(HashPasswordRequest {
            password: String::new(),
        });
        let result = service.hash_password(hash_req).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_verify_invalid_hash() {
        let service = PasswordServiceImpl::new();

        let verify_req = Request::new(VerifyPasswordRequest {
            password: "password".to_string(),
            hash: "invalid-hash-format".to_string(),
        });
        let verify_resp = service.verify_password(verify_req).await.unwrap();
        assert!(!verify_resp.into_inner().valid);
    }

    #[tokio::test]
    async fn test_custom_params() {
        let service = PasswordServiceImpl::with_params(
            19456, // memory cost in KiB
            2,     // time cost
            1,     // parallelism
            Some(32), // output length
        );

        let hash_req = Request::new(HashPasswordRequest {
            password: "testpassword".to_string(),
        });
        let hash_resp = service.hash_password(hash_req).await.unwrap();
        let hash = hash_resp.into_inner().hash;

        // Hash should start with argon2id identifier
        assert!(hash.starts_with("$argon2id$"));
    }
}

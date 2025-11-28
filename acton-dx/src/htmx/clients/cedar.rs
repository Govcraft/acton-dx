//! Cedar authorization service client.

use super::error::ClientError;
use acton_dx_proto::cedar::v1::{
    cedar_service_client::CedarServiceClient, AuthzRequest, BatchAuthzRequest, Entity,
    ReloadPoliciesRequest, ValidatePolicyRequest,
};
use std::collections::HashMap;
use tonic::transport::Channel;

/// Client for the Cedar authorization service.
///
/// Provides Cedar policy-based authorization checks with batch support.
#[derive(Debug, Clone)]
pub struct CedarClient {
    client: CedarServiceClient<Channel>,
}

impl CedarClient {
    /// Connect to the Cedar service.
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
            client: CedarServiceClient::new(channel),
        })
    }

    /// Check if an action is authorized.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn is_authorized(
        &mut self,
        principal_type: &str,
        principal_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        context: HashMap<String, String>,
    ) -> Result<AuthorizationResult, ClientError> {
        let response = self
            .client
            .is_authorized(AuthzRequest {
                principal: Some(Entity {
                    entity_type: principal_type.to_string(),
                    entity_id: principal_id.to_string(),
                }),
                action: action.to_string(),
                resource: Some(Entity {
                    entity_type: resource_type.to_string(),
                    entity_id: resource_id.to_string(),
                }),
                context,
            })
            .await?;

        let inner = response.into_inner();
        Ok(AuthorizationResult {
            allowed: inner.allowed,
            decision_reason: inner.decision_reason,
            diagnostics: inner.diagnostics,
        })
    }

    /// Check authorization for multiple requests in batch.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn batch_authorize(
        &mut self,
        requests: Vec<AuthorizationRequest>,
    ) -> Result<Vec<AuthorizationResult>, ClientError> {
        let proto_requests: Vec<AuthzRequest> = requests
            .into_iter()
            .map(|r| AuthzRequest {
                principal: Some(Entity {
                    entity_type: r.principal_type,
                    entity_id: r.principal_id,
                }),
                action: r.action,
                resource: Some(Entity {
                    entity_type: r.resource_type,
                    entity_id: r.resource_id,
                }),
                context: r.context,
            })
            .collect();

        let response = self
            .client
            .batch_authorize(BatchAuthzRequest {
                requests: proto_requests,
            })
            .await?;

        Ok(response
            .into_inner()
            .responses
            .into_iter()
            .map(|r| AuthorizationResult {
                allowed: r.allowed,
                decision_reason: r.decision_reason,
                diagnostics: r.diagnostics,
            })
            .collect())
    }

    /// Reload Cedar policies.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn reload_policies(&mut self) -> Result<ReloadResult, ClientError> {
        let response = self
            .client
            .reload_policies(ReloadPoliciesRequest {})
            .await?;

        let inner = response.into_inner();
        Ok(ReloadResult {
            success: inner.success,
            policies_loaded: inner.policies_loaded,
            message: inner.message,
        })
    }

    /// Validate a Cedar policy.
    ///
    /// # Errors
    ///
    /// Returns error if the service call fails.
    pub async fn validate_policy(
        &mut self,
        policy_text: &str,
    ) -> Result<ValidationResult, ClientError> {
        let response = self
            .client
            .validate_policy(ValidatePolicyRequest {
                policy_text: policy_text.to_string(),
            })
            .await?;

        let inner = response.into_inner();
        Ok(ValidationResult {
            valid: inner.valid,
            errors: inner.errors,
        })
    }
}

/// Authorization request for batch operations.
#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    /// Principal entity type.
    pub principal_type: String,
    /// Principal entity ID.
    pub principal_id: String,
    /// Action to check.
    pub action: String,
    /// Resource entity type.
    pub resource_type: String,
    /// Resource entity ID.
    pub resource_id: String,
    /// Additional context.
    pub context: HashMap<String, String>,
}

/// Result of an authorization check.
#[derive(Debug, Clone)]
pub struct AuthorizationResult {
    /// Whether the action is allowed.
    pub allowed: bool,
    /// Reason for the decision.
    pub decision_reason: String,
    /// Diagnostic messages.
    pub diagnostics: Vec<String>,
}

/// Result of a policy reload.
#[derive(Debug, Clone)]
pub struct ReloadResult {
    /// Whether the reload succeeded.
    pub success: bool,
    /// Number of policies loaded.
    pub policies_loaded: i32,
    /// Status message.
    pub message: String,
}

/// Result of policy validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the policy is valid.
    pub valid: bool,
    /// Validation errors.
    pub errors: Vec<String>,
}

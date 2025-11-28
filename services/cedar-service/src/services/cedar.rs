//! Cedar authorization service gRPC implementation.

use acton_dx_proto::cedar::v1::{
    cedar_service_server::CedarService, AuthzRequest, AuthzResponse, BatchAuthzRequest,
    BatchAuthzResponse, Entity, ReloadPoliciesRequest, ReloadPoliciesResponse,
    ValidatePolicyRequest, ValidatePolicyResponse,
};
use cedar_policy::{Authorizer, Context, Entities, EntityUid, PolicySet, Request};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tonic::{Request as TonicRequest, Response, Status};
use tracing::{debug, error, info, warn};

/// Cedar authorization service implementation.
pub struct CedarServiceImpl {
    /// The Cedar authorizer.
    authorizer: Authorizer,
    /// The loaded policy set (protected by RwLock for hot reloading).
    policies: Arc<RwLock<PolicySet>>,
    /// The entities (protected by RwLock).
    entities: Arc<RwLock<Entities>>,
    /// Path to policies directory.
    policies_path: String,
}

/// Error creating an authorization response.
#[derive(Debug)]
struct AuthzError {
    reason: String,
}

impl AuthzError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    fn into_response(self) -> AuthzResponse {
        AuthzResponse {
            allowed: false,
            decision_reason: self.reason,
            diagnostics: vec![],
        }
    }
}

impl CedarServiceImpl {
    /// Create a new Cedar service with policies from the given path.
    ///
    /// # Errors
    ///
    /// Returns error if policies cannot be loaded.
    pub fn new(policies_path: &str) -> anyhow::Result<Self> {
        let policies = Self::load_policies_from_path(policies_path)?;
        let policy_count = policies.policies().count();

        info!(
            path = %policies_path,
            policies = policy_count,
            "Loaded Cedar policies"
        );

        Ok(Self {
            authorizer: Authorizer::new(),
            policies: Arc::new(RwLock::new(policies)),
            entities: Arc::new(RwLock::new(Entities::empty())),
            policies_path: policies_path.to_string(),
        })
    }

    /// Create a new Cedar service with an empty policy set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            authorizer: Authorizer::new(),
            policies: Arc::new(RwLock::new(PolicySet::new())),
            entities: Arc::new(RwLock::new(Entities::empty())),
            policies_path: String::new(),
        }
    }

    /// Load policies from a directory path.
    fn load_policies_from_path(path: &str) -> anyhow::Result<PolicySet> {
        let path = Path::new(path);

        if !path.exists() {
            warn!(path = %path.display(), "Policies path does not exist, using empty policy set");
            return Ok(PolicySet::new());
        }

        if path.is_file() {
            let content = std::fs::read_to_string(path)?;
            return Ok(content.parse()?);
        }

        Self::load_policies_from_directory(path)
    }

    /// Load policies from a directory.
    fn load_policies_from_directory(path: &Path) -> anyhow::Result<PolicySet> {
        let mut all_content = String::new();

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.extension().is_some_and(|ext| ext == "cedar") {
                let content = std::fs::read_to_string(&file_path)?;
                all_content.push_str(&content);
                all_content.push('\n');
            }
        }

        Ok(all_content.parse()?)
    }

    /// Convert a proto Entity to a Cedar `EntityUid`.
    fn entity_to_uid(entity: &Entity) -> Result<EntityUid, AuthzError> {
        let entity_str = format!("{}::\"{}\"", entity.entity_type, entity.entity_id);
        entity_str.parse().map_err(|e| {
            error!(error = %e, entity = %entity_str, "Invalid entity format");
            AuthzError::new(format!("Invalid entity: {e}"))
        })
    }

    /// Convert context map to Cedar Context.
    fn build_context(context: &HashMap<String, String>) -> Result<Context, AuthzError> {
        if context.is_empty() {
            return Ok(Context::empty());
        }

        let json_value: serde_json::Value = context
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();

        Context::from_json_value(json_value, None).map_err(|e| {
            error!(error = %e, "Failed to build context");
            AuthzError::new(format!("Invalid context: {e}"))
        })
    }

    /// Parse principal from request.
    fn parse_principal(req: &AuthzRequest) -> Result<EntityUid, AuthzError> {
        req.principal
            .as_ref()
            .ok_or_else(|| AuthzError::new("Missing principal"))
            .and_then(Self::entity_to_uid)
    }

    /// Parse action from request.
    fn parse_action(action: &str) -> Result<EntityUid, AuthzError> {
        let action_str = format!("Action::\"{action}\"");
        action_str
            .parse()
            .map_err(|e| AuthzError::new(format!("Invalid action: {e}")))
    }

    /// Parse resource from request.
    fn parse_resource(req: &AuthzRequest) -> Result<EntityUid, AuthzError> {
        req.resource
            .as_ref()
            .ok_or_else(|| AuthzError::new("Missing resource"))
            .and_then(Self::entity_to_uid)
    }

    /// Build a Cedar request from the proto request.
    fn build_cedar_request(req: &AuthzRequest) -> Result<Request, AuthzError> {
        let principal = Self::parse_principal(req)?;
        let action = Self::parse_action(&req.action)?;
        let resource = Self::parse_resource(req)?;
        let context = Self::build_context(&req.context)?;

        Request::new(principal, action, resource, context, None)
            .map_err(|e| AuthzError::new(format!("Invalid request: {e}")))
    }

    /// Execute authorization and build response.
    fn execute_authorization(&self, cedar_request: &Request, req: &AuthzRequest) -> AuthzResponse {
        let policies = self.policies.read();
        let entities = self.entities.read();
        let response = self.authorizer.is_authorized(cedar_request, &policies, &entities);
        drop(policies);
        drop(entities);

        let allowed = response.decision() == cedar_policy::Decision::Allow;
        let diagnostics: Vec<String> = response
            .diagnostics()
            .errors()
            .map(ToString::to_string)
            .collect();

        debug!(
            principal = %req.principal.as_ref().map_or("none", |p| p.entity_id.as_str()),
            action = %req.action,
            resource = %req.resource.as_ref().map_or("none", |r| r.entity_id.as_str()),
            allowed = %allowed,
            "Authorization decision"
        );

        AuthzResponse {
            allowed,
            decision_reason: if allowed {
                "Allowed by policy".to_string()
            } else {
                "Denied by policy".to_string()
            },
            diagnostics,
        }
    }

    /// Perform a single authorization check.
    fn authorize_single(&self, req: &AuthzRequest) -> AuthzResponse {
        match Self::build_cedar_request(req) {
            Ok(cedar_request) => self.execute_authorization(&cedar_request, req),
            Err(e) => e.into_response(),
        }
    }

    /// Safely convert usize to i32.
    fn usize_to_i32(value: usize) -> i32 {
        i32::try_from(value).unwrap_or(i32::MAX)
    }
}

#[tonic::async_trait]
impl CedarService for CedarServiceImpl {
    async fn is_authorized(
        &self,
        request: TonicRequest<AuthzRequest>,
    ) -> Result<Response<AuthzResponse>, Status> {
        let req = request.into_inner();
        let response = self.authorize_single(&req);
        Ok(Response::new(response))
    }

    async fn batch_authorize(
        &self,
        request: TonicRequest<BatchAuthzRequest>,
    ) -> Result<Response<BatchAuthzResponse>, Status> {
        let req = request.into_inner();
        let responses: Vec<AuthzResponse> = req
            .requests
            .iter()
            .map(|r| self.authorize_single(r))
            .collect();

        Ok(Response::new(BatchAuthzResponse { responses }))
    }

    async fn reload_policies(
        &self,
        _request: TonicRequest<ReloadPoliciesRequest>,
    ) -> Result<Response<ReloadPoliciesResponse>, Status> {
        if self.policies_path.is_empty() {
            return Ok(Response::new(ReloadPoliciesResponse {
                success: false,
                policies_loaded: 0,
                message: "No policies path configured".to_string(),
            }));
        }

        match Self::load_policies_from_path(&self.policies_path) {
            Ok(new_policies) => {
                let count = new_policies.policies().count();
                *self.policies.write() = new_policies;
                info!(policies = count, "Reloaded Cedar policies");
                Ok(Response::new(ReloadPoliciesResponse {
                    success: true,
                    policies_loaded: Self::usize_to_i32(count),
                    message: format!("Loaded {count} policies"),
                }))
            }
            Err(e) => {
                error!(error = %e, "Failed to reload policies");
                Ok(Response::new(ReloadPoliciesResponse {
                    success: false,
                    policies_loaded: 0,
                    message: format!("Failed to reload: {e}"),
                }))
            }
        }
    }

    async fn validate_policy(
        &self,
        request: TonicRequest<ValidatePolicyRequest>,
    ) -> Result<Response<ValidatePolicyResponse>, Status> {
        let req = request.into_inner();

        match req.policy_text.parse::<PolicySet>() {
            Ok(_) => Ok(Response::new(ValidatePolicyResponse {
                valid: true,
                errors: vec![],
            })),
            Err(e) => Ok(Response::new(ValidatePolicyResponse {
                valid: false,
                errors: vec![e.to_string()],
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_policy_set() {
        let service = CedarServiceImpl::empty();
        assert!(service.policies.read().is_empty());
    }

    #[test]
    fn test_entity_to_uid() {
        let entity = Entity {
            entity_type: "User".to_string(),
            entity_id: "alice".to_string(),
        };
        let uid = CedarServiceImpl::entity_to_uid(&entity).unwrap();
        assert_eq!(uid.to_string(), "User::\"alice\"");
    }

    #[test]
    fn test_authorize_missing_principal() {
        let service = CedarServiceImpl::empty();
        let req = AuthzRequest {
            principal: None,
            action: "read".to_string(),
            resource: Some(Entity {
                entity_type: "Document".to_string(),
                entity_id: "doc1".to_string(),
            }),
            context: HashMap::new(),
        };
        let response = service.authorize_single(&req);
        assert!(!response.allowed);
        assert!(response.decision_reason.contains("Missing principal"));
    }

    #[test]
    fn test_safe_conversion() {
        assert_eq!(CedarServiceImpl::usize_to_i32(100), 100);
    }
}

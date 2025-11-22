//! HTTP handlers for acton-htmx
//!
//! This module contains HTTP request handlers for various features:
//! - Cedar policy administration (admin-only endpoints)
//! - Role management (admin-only endpoints)
//! - Job management (admin-only endpoints)

#[cfg(feature = "cedar")]
pub mod cedar_admin;
pub mod job_admin;
pub mod role_admin;

// Re-exports
#[cfg(feature = "cedar")]
#[allow(unused_imports)]
pub use cedar_admin::{policy_status, reload_policies, PolicyStatusResponse, ReloadPolicyResponse};

#[allow(unused_imports)]
pub use job_admin::{job_stats, list_jobs, JobListResponse, JobStatsResponse};

#[allow(unused_imports)]
pub use role_admin::{
    assign_role, get_user_roles, remove_role, AssignRoleRequest, RoleResponse,
};

//! Job management admin handlers
//!
//! This module provides HTTP handlers for managing background jobs.
//! These handlers should be protected with admin-only authorization.
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use acton_htmx::handlers::job_admin;
//! use axum::Router;
//!
//! let admin_routes = Router::new()
//!     .route("/admin/jobs/list", get(job_admin::list_jobs))
//!     .route("/admin/jobs/stats", get(job_admin::job_stats));
//! ```

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::{user::User, Authenticated};
use crate::state::ActonHtmxState;

/// Response for job list endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct JobListResponse {
    /// List of jobs
    pub jobs: Vec<JobInfo>,
    /// Total number of jobs matching filters
    pub total: usize,
    /// Success message
    pub message: String,
}

/// Information about a single job
#[derive(Debug, Serialize, Deserialize)]
pub struct JobInfo {
    /// Job ID
    pub id: String,
    /// Job type
    pub job_type: String,
    /// Current status
    pub status: String,
    /// When the job was created
    pub created_at: String,
    /// Job priority
    pub priority: i32,
}

/// Response for job statistics endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct JobStatsResponse {
    /// Total jobs enqueued
    pub total_enqueued: u64,
    /// Currently running jobs
    pub running: usize,
    /// Pending jobs in queue
    pub pending: usize,
    /// Completed jobs
    pub completed: u64,
    /// Failed jobs
    pub failed: u64,
    /// Jobs in dead letter queue
    pub dead_letter: u64,
    /// Average execution time in milliseconds
    pub avg_execution_ms: f64,
    /// P95 execution time in milliseconds
    pub p95_execution_ms: f64,
    /// P99 execution time in milliseconds
    pub p99_execution_ms: f64,
    /// Success rate as percentage
    pub success_rate: f64,
    /// Message
    pub message: String,
}

/// List all jobs
///
/// Returns a list of jobs from the queue and their current status.
/// Requires admin role.
///
/// # Example
///
/// ```bash
/// GET /admin/jobs/list
/// ```
///
/// Response:
/// ```json
/// {
///   "jobs": [],
///   "total": 0,
///   "message": "Jobs retrieved successfully"
/// }
/// ```
pub async fn list_jobs(
    State(_state): State<ActonHtmxState>,
    Authenticated(admin): Authenticated<User>,
) -> Result<Response, StatusCode> {
    // Verify admin role
    if !admin.roles.contains(&"admin".to_string()) {
        tracing::warn!(
            admin_id = admin.id,
            "Non-admin attempted to list jobs"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // For now, we return empty list as we don't have a message to list all jobs
    // This would require adding a new message type to the JobAgent
    // In Phase 3, we can add ListJobs message to get actual job data

    let response = JobListResponse {
        jobs: vec![],
        total: 0,
        message: "Job listing functionality will be enhanced in Phase 3".to_string(),
    };

    tracing::info!(
        admin_id = admin.id,
        "Admin retrieved job list"
    );

    Ok((StatusCode::OK, Json(response)).into_response())
}

/// Get job statistics
///
/// Returns comprehensive statistics about the job queue and execution metrics.
/// Requires admin role.
///
/// # Example
///
/// ```bash
/// GET /admin/jobs/stats
/// ```
///
/// Response:
/// ```json
/// {
///   "total_enqueued": 150,
///   "running": 2,
///   "pending": 5,
///   "completed": 140,
///   "failed": 3,
///   "dead_letter": 0,
///   "avg_execution_ms": 125.5,
///   "p95_execution_ms": 450.0,
///   "p99_execution_ms": 890.0,
///   "success_rate": 97.9,
///   "message": "Statistics retrieved successfully"
/// }
/// ```
#[allow(clippy::cast_precision_loss)] // Acceptable for metrics
pub async fn job_stats(
    State(state): State<ActonHtmxState>,
    Authenticated(admin): Authenticated<User>,
) -> Result<Response, StatusCode> {
    // Verify admin role
    if !admin.roles.contains(&"admin".to_string()) {
        tracing::warn!(
            admin_id = admin.id,
            "Non-admin attempted to view job statistics"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Get metrics from job agent
    let _job_agent = state.job_agent();

    // TODO: Implement message-based communication with JobAgent
    // For now, return placeholder metrics
    // This requires making GetMetrics public and implementing proper message handling

    // Placeholder metrics for initial implementation
    let total_processed = 0_u64;
    let success_rate = if total_processed > 0 {
        0.0
    } else {
        100.0
    };

    let response = JobStatsResponse {
        total_enqueued: 0,
        running: 0,
        pending: 0,
        completed: 0,
        failed: 0,
        dead_letter: 0,
        avg_execution_ms: 0.0,
        p95_execution_ms: 0.0,
        p99_execution_ms: 0.0,
        success_rate,
        message: "Statistics retrieved successfully (placeholder data)".to_string(),
    };

    tracing::info!(
        admin_id = admin.id,
        "Admin retrieved job statistics"
    );

    Ok((StatusCode::OK, Json(response)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_info_serialization() {
        let job = JobInfo {
            id: "job-123".to_string(),
            job_type: "WelcomeEmail".to_string(),
            status: "pending".to_string(),
            created_at: "2025-11-22T10:00:00Z".to_string(),
            priority: 10,
        };

        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("job-123"));
        assert!(json.contains("WelcomeEmail"));
    }

    #[test]
    fn test_job_stats_response_serialization() {
        let stats = JobStatsResponse {
            total_enqueued: 100,
            running: 2,
            pending: 5,
            completed: 90,
            failed: 3,
            dead_letter: 0,
            avg_execution_ms: 125.5,
            p95_execution_ms: 450.0,
            p99_execution_ms: 890.0,
            success_rate: 96.8,
            message: "Success".to_string(),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"total_enqueued\":100"));
        assert!(json.contains("\"running\":2"));
        assert!(json.contains("\"success_rate\":96.8"));
    }
}

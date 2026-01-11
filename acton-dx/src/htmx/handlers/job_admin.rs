//! Job management admin handlers
//!
//! This module provides HTTP handlers for managing background jobs.
//! These handlers should be protected with admin-only authorization.
//!
//! # Architecture
//!
//! Uses acton-reactive web handler pattern with oneshot channels for
//! request-reply communication between HTTP handlers and the `JobAgent`.
//!
//! See `.claude/acton-reactive-research-20251124-message-patterns.md` for
//! detailed documentation on the message passing patterns.
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

use acton_reactive::prelude::ActorHandleInterface;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::htmx::auth::{user::User, Authenticated};
use crate::htmx::jobs::{
    agent::{
        CancelJobRequest, ClearDeadLetterQueueRequest, GetMetricsRequest, RetryAllFailedRequest,
        RetryJobRequest,
    },
    JobId,
};
use crate::htmx::state::ActonHtmxState;

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
/// # Errors
///
/// Returns [`StatusCode::FORBIDDEN`] if the authenticated user does not have the "admin" role.
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
/// Uses acton-reactive web handler pattern with oneshot channel for
/// communication with `JobAgent`. Includes 100ms timeout to prevent handler
/// from hanging if the agent is slow or stopped.
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
///
/// # Errors
///
/// Returns:
/// - `403 FORBIDDEN` if user is not an admin
/// - `408 REQUEST_TIMEOUT` if agent doesn't respond within 100ms
/// - `500 INTERNAL_SERVER_ERROR` if agent response channel fails
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

    // Create request with response channel (web handler pattern)
    let (request, rx) = GetMetricsRequest::new();

    // Send message to JobAgent (fire-and-forget from handler perspective)
    state.job_agent().send(request).await;

    // Await response with 100ms timeout
    let timeout = Duration::from_millis(100);
    let metrics = tokio::time::timeout(timeout, rx)
        .await
        .map_err(|_| {
            tracing::error!("Job metrics retrieval timeout");
            StatusCode::REQUEST_TIMEOUT
        })?
        .map_err(|_| {
            tracing::error!("Job metrics channel error");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Calculate success rate from metrics
    let total_processed = metrics.jobs_completed + metrics.jobs_failed;
    let success_rate = if total_processed > 0 {
        (metrics.jobs_completed as f64 / total_processed as f64) * 100.0
    } else {
        100.0
    };

    // Build response from real metrics
    let response = JobStatsResponse {
        total_enqueued: metrics.jobs_enqueued,
        running: metrics.current_running,
        pending: metrics.current_queue_size,
        completed: metrics.jobs_completed,
        failed: metrics.jobs_failed,
        dead_letter: metrics.jobs_in_dlq,
        avg_execution_ms: metrics.avg_execution_time_ms as f64,
        p95_execution_ms: metrics.p95_execution_time_ms as f64,
        p99_execution_ms: metrics.p99_execution_time_ms as f64,
        success_rate,
        message: "Statistics retrieved successfully".to_string(),
    };

    tracing::info!(
        admin_id = admin.id,
        jobs_enqueued = metrics.jobs_enqueued,
        jobs_completed = metrics.jobs_completed,
        jobs_failed = metrics.jobs_failed,
        "Admin retrieved job statistics"
    );

    Ok((StatusCode::OK, Json(response)).into_response())
}

/// Retry a failed job by ID
///
/// Re-queues a job from the dead letter queue back into the main queue
/// for another execution attempt. Requires admin role.
///
/// # Example
///
/// ```bash
/// POST /admin/jobs/{job_id}/retry
/// ```
///
/// Response:
/// ```json
/// {
///   "success": true,
///   "message": "Job queued for retry"
/// }
/// ```
///
/// # Errors
///
/// Returns:
/// - `403 FORBIDDEN` if user is not an admin
/// - `404 NOT_FOUND` if job is not in dead letter queue
/// - `408 REQUEST_TIMEOUT` if agent doesn't respond within 100ms
/// - `500 INTERNAL_SERVER_ERROR` if agent response channel fails
pub async fn retry_job(
    State(state): State<ActonHtmxState>,
    Authenticated(admin): Authenticated<User>,
    Path(job_id): Path<JobId>,
) -> Result<Response, StatusCode> {
    // Verify admin role
    if !admin.roles.contains(&"admin".to_string()) {
        tracing::warn!(
            admin_id = admin.id,
            %job_id,
            "Non-admin attempted to retry job"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Create request with response channel
    let (request, rx) = RetryJobRequest::new(job_id);

    // Send message to JobAgent
    state.job_agent().send(request).await;

    // Await response with 100ms timeout
    let timeout = Duration::from_millis(100);
    let success = tokio::time::timeout(timeout, rx)
        .await
        .map_err(|_| {
            tracing::error!(%job_id, "Job retry timeout");
            StatusCode::REQUEST_TIMEOUT
        })?
        .map_err(|_| {
            tracing::error!(%job_id, "Job retry channel error");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if success {
        tracing::info!(
            admin_id = admin.id,
            %job_id,
            "Job queued for retry"
        );

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": "Job queued for retry"
            })),
        )
            .into_response())
    } else {
        tracing::warn!(
            admin_id = admin.id,
            %job_id,
            "Job not found in dead letter queue"
        );
        Err(StatusCode::NOT_FOUND)
    }
}

/// Retry all failed jobs
///
/// Re-queues all jobs from the dead letter queue back into the main queue.
/// Requires admin role.
///
/// # Example
///
/// ```bash
/// POST /admin/jobs/retry-all
/// ```
///
/// Response:
/// ```json
/// {
///   "retried": 5,
///   "message": "5 jobs queued for retry"
/// }
/// ```
///
/// # Errors
///
/// Returns:
/// - `403 FORBIDDEN` if user is not an admin
/// - `408 REQUEST_TIMEOUT` if agent doesn't respond within 500ms
/// - `500 INTERNAL_SERVER_ERROR` if agent response channel fails
pub async fn retry_all_jobs(
    State(state): State<ActonHtmxState>,
    Authenticated(admin): Authenticated<User>,
) -> Result<Response, StatusCode> {
    // Verify admin role
    if !admin.roles.contains(&"admin".to_string()) {
        tracing::warn!(
            admin_id = admin.id,
            "Non-admin attempted to retry all jobs"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Create request with response channel
    let (request, rx) = RetryAllFailedRequest::new();

    // Send message to JobAgent
    state.job_agent().send(request).await;

    // Await response with 500ms timeout (may need to requeue many jobs)
    let timeout = Duration::from_millis(500);
    let retried = tokio::time::timeout(timeout, rx)
        .await
        .map_err(|_| {
            tracing::error!("Retry all jobs timeout");
            StatusCode::REQUEST_TIMEOUT
        })?
        .map_err(|_| {
            tracing::error!("Retry all jobs channel error");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(
        admin_id = admin.id,
        retried,
        "All failed jobs queued for retry"
    );

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "retried": retried,
            "message": format!("{retried} jobs queued for retry")
        })),
    )
        .into_response())
}

/// Cancel a running or pending job
///
/// Attempts to cancel a job. If the job is pending, it's removed from the queue.
/// If it's currently running, a cancellation signal is sent.
/// Requires admin role.
///
/// # Example
///
/// ```bash
/// POST /admin/jobs/{job_id}/cancel
/// ```
///
/// Response:
/// ```json
/// {
///   "success": true,
///   "message": "Job cancellation requested"
/// }
/// ```
///
/// # Errors
///
/// Returns:
/// - `403 FORBIDDEN` if user is not an admin
/// - `404 NOT_FOUND` if job is not found
/// - `408 REQUEST_TIMEOUT` if agent doesn't respond within 100ms
/// - `500 INTERNAL_SERVER_ERROR` if agent response channel fails
pub async fn cancel_job(
    State(state): State<ActonHtmxState>,
    Authenticated(admin): Authenticated<User>,
    Path(job_id): Path<JobId>,
) -> Result<Response, StatusCode> {
    // Verify admin role
    if !admin.roles.contains(&"admin".to_string()) {
        tracing::warn!(
            admin_id = admin.id,
            %job_id,
            "Non-admin attempted to cancel job"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Create request with response channel
    let (request, rx) = CancelJobRequest::new(job_id);

    // Send message to JobAgent
    state.job_agent().send(request).await;

    // Await response with 100ms timeout
    let timeout = Duration::from_millis(100);
    let success = tokio::time::timeout(timeout, rx)
        .await
        .map_err(|_| {
            tracing::error!(%job_id, "Job cancel timeout");
            StatusCode::REQUEST_TIMEOUT
        })?
        .map_err(|_| {
            tracing::error!(%job_id, "Job cancel channel error");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if success {
        tracing::info!(
            admin_id = admin.id,
            %job_id,
            "Job cancellation requested"
        );

        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": "Job cancellation requested"
            })),
        )
            .into_response())
    } else {
        tracing::warn!(
            admin_id = admin.id,
            %job_id,
            "Job not found"
        );
        Err(StatusCode::NOT_FOUND)
    }
}

/// Clear the dead letter queue
///
/// Permanently removes all jobs from the dead letter queue.
/// This operation cannot be undone. Requires admin role.
///
/// # Example
///
/// ```bash
/// POST /admin/jobs/dead-letter/clear
/// ```
///
/// Response:
/// ```json
/// {
///   "cleared": 3,
///   "message": "3 jobs removed from dead letter queue"
/// }
/// ```
///
/// # Errors
///
/// Returns:
/// - `403 FORBIDDEN` if user is not an admin
/// - `408 REQUEST_TIMEOUT` if agent doesn't respond within 100ms
/// - `500 INTERNAL_SERVER_ERROR` if agent response channel fails
pub async fn clear_dead_letter_queue(
    State(state): State<ActonHtmxState>,
    Authenticated(admin): Authenticated<User>,
) -> Result<Response, StatusCode> {
    // Verify admin role
    if !admin.roles.contains(&"admin".to_string()) {
        tracing::warn!(
            admin_id = admin.id,
            "Non-admin attempted to clear dead letter queue"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Create request with response channel
    let (request, rx) = ClearDeadLetterQueueRequest::new();

    // Send message to JobAgent
    state.job_agent().send(request).await;

    // Await response with 100ms timeout
    let timeout = Duration::from_millis(100);
    let cleared = tokio::time::timeout(timeout, rx)
        .await
        .map_err(|_| {
            tracing::error!("Clear dead letter queue timeout");
            StatusCode::REQUEST_TIMEOUT
        })?
        .map_err(|_| {
            tracing::error!("Clear dead letter queue channel error");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(
        admin_id = admin.id,
        cleared,
        "Dead letter queue cleared"
    );

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "cleared": cleared,
            "message": format!("{cleared} jobs removed from dead letter queue")
        })),
    )
        .into_response())
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

//! Job admin UI and job template generation

/// Job admin dashboard template (HTMX-based)
pub const JOB_DASHBOARD_TEMPLATE: &str = r##"{% extends "layouts/app.html" %}

{% block title %}Job Dashboard - {{project_name}}{% endblock %}

{% block content %}
<div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
    <div class="mb-8">
        <h1 class="text-3xl font-bold text-gray-900">Background Jobs</h1>
        <p class="mt-2 text-sm text-gray-600">Monitor and manage background job execution</p>
    </div>

    <!-- Statistics Cards -->
    <div class="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-4 mb-8"
         hx-get="/admin/jobs/stats"
         hx-trigger="every 5s"
         hx-swap="innerHTML">
        {% include "jobs/_stats.html" %}
    </div>

    <!-- Job Queue Tables -->
    <div class="space-y-8">
        <!-- Running Jobs -->
        <section>
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-semibold text-gray-900">Running Jobs</h2>
                <span class="text-sm text-gray-500" id="running-count">{{ running_count }} active</span>
            </div>
            <div hx-get="/admin/jobs/running"
                 hx-trigger="every 2s"
                 hx-swap="innerHTML">
                {% include "jobs/_running_list.html" %}
            </div>
        </section>

        <!-- Pending Jobs -->
        <section>
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-semibold text-gray-900">Pending Jobs</h2>
                <span class="text-sm text-gray-500" id="pending-count">{{ pending_count }} queued</span>
            </div>
            <div hx-get="/admin/jobs/pending"
                 hx-trigger="every 5s"
                 hx-swap="innerHTML">
                {% include "jobs/_pending_list.html" %}
            </div>
        </section>

        <!-- Failed Jobs -->
        <section>
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-semibold text-gray-900">Failed Jobs</h2>
                <div class="flex items-center space-x-4">
                    <span class="text-sm text-gray-500" id="failed-count">{{ failed_count }} failed</span>
                    <button hx-post="/admin/jobs/retry-all"
                            hx-confirm="Retry all failed jobs?"
                            class="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700">
                        Retry All
                    </button>
                    <button hx-delete="/admin/jobs/dead-letter"
                            hx-confirm="Clear dead letter queue? This cannot be undone."
                            class="px-3 py-1 text-sm bg-red-600 text-white rounded hover:bg-red-700">
                        Clear DLQ
                    </button>
                </div>
            </div>
            <div hx-get="/admin/jobs/failed"
                 hx-trigger="every 10s"
                 hx-swap="innerHTML">
                {% include "jobs/_failed_list.html" %}
            </div>
        </section>

        <!-- Job History -->
        <section>
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-xl font-semibold text-gray-900">Recent History</h2>
                <div class="flex items-center space-x-2">
                    <input type="text"
                           name="search"
                           placeholder="Search jobs..."
                           hx-get="/admin/jobs/history"
                           hx-trigger="keyup changed delay:500ms"
                           hx-target="#job-history"
                           class="px-3 py-1 border border-gray-300 rounded text-sm">
                </div>
            </div>
            <div id="job-history"
                 hx-get="/admin/jobs/history"
                 hx-trigger="load"
                 hx-swap="innerHTML">
                {% include "jobs/_history_list.html" %}
            </div>
        </section>
    </div>

    <!-- Performance Charts -->
    <div class="mt-8 grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div class="bg-white p-6 rounded-lg shadow">
            <h3 class="text-lg font-semibold mb-4">Execution Time (P95)</h3>
            <div hx-get="/admin/jobs/charts/execution-time"
                 hx-trigger="every 30s">
                Loading chart...
            </div>
        </div>
        <div class="bg-white p-6 rounded-lg shadow">
            <h3 class="text-lg font-semibold mb-4">Success Rate</h3>
            <div hx-get="/admin/jobs/charts/success-rate"
                 hx-trigger="every 30s">
                Loading chart...
            </div>
        </div>
    </div>
</div>
{% endblock %}
"##;

/// Job statistics partial
pub const JOB_STATS_PARTIAL: &str = r#"<div class="bg-white overflow-hidden shadow rounded-lg">
    <div class="px-4 py-5 sm:p-6">
        <dt class="text-sm font-medium text-gray-500 truncate">Total Jobs</dt>
        <dd class="mt-1 text-3xl font-semibold text-gray-900">{{ total_jobs }}</dd>
    </div>
</div>

<div class="bg-white overflow-hidden shadow rounded-lg">
    <div class="px-4 py-5 sm:p-6">
        <dt class="text-sm font-medium text-gray-500 truncate">Running</dt>
        <dd class="mt-1 text-3xl font-semibold text-blue-600">{{ running_jobs }}</dd>
    </div>
</div>

<div class="bg-white overflow-hidden shadow rounded-lg">
    <div class="px-4 py-5 sm:p-6">
        <dt class="text-sm font-medium text-gray-500 truncate">Completed</dt>
        <dd class="mt-1 text-3xl font-semibold text-green-600">{{ completed_jobs }}</dd>
    </div>
</div>

<div class="bg-white overflow-hidden shadow rounded-lg">
    <div class="px-4 py-5 sm:p-6">
        <dt class="text-sm font-medium text-gray-500 truncate">Failed</dt>
        <dd class="mt-1 text-3xl font-semibold text-red-600">{{ failed_jobs }}</dd>
    </div>
</div>
"#;

/// Running jobs list partial
pub const JOB_RUNNING_LIST: &str = r#"{% if jobs.is_empty() %}
<div class="text-center py-12 bg-white rounded-lg shadow">
    <svg class="mx-auto h-12 w-12 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
    </svg>
    <h3 class="mt-2 text-sm font-medium text-gray-900">No running jobs</h3>
    <p class="mt-1 text-sm text-gray-500">All jobs have completed or are pending</p>
</div>
{% else %}
<div class="bg-white shadow overflow-hidden sm:rounded-lg">
    <table class="min-w-full divide-y divide-gray-200">
        <thead class="bg-gray-50">
            <tr>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Job ID</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Type</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Started</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Duration</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Actions</th>
            </tr>
        </thead>
        <tbody class="bg-white divide-y divide-gray-200">
            {% for job in jobs %}
            <tr id="job-{{ job.id }}">
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">
                    <code class="bg-gray-100 px-2 py-1 rounded">{{ job.id }}</code>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.job_type }}</td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.started_at }}</td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                    <span class="inline-flex items-center">
                        <span class="animate-pulse">●</span>
                        <span class="ml-2">{{ job.duration }}</span>
                    </span>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium">
                    <button hx-post="/admin/jobs/{{ job.id }}/cancel"
                            hx-confirm="Cancel this job?"
                            hx-target="closest tr"
                            hx-swap="outerHTML"
                            class="text-red-600 hover:text-red-900">
                        Cancel
                    </button>
                </td>
            </tr>
            {% endfor %}
        </tbody>
    </table>
</div>
{% endif %}
"#;

/// Failed jobs list partial
pub const JOB_FAILED_LIST: &str = r#"{% if jobs.is_empty() %}
<div class="text-center py-12 bg-white rounded-lg shadow">
    <svg class="mx-auto h-12 w-12 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"/>
    </svg>
    <h3 class="mt-2 text-sm font-medium text-gray-900">No failed jobs</h3>
    <p class="mt-1 text-sm text-gray-500">All jobs completed successfully</p>
</div>
{% else %}
<div class="bg-white shadow overflow-hidden sm:rounded-lg">
    <table class="min-w-full divide-y divide-gray-200">
        <thead class="bg-gray-50">
            <tr>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Job ID</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Type</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Failed At</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Error</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Retries</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Actions</th>
            </tr>
        </thead>
        <tbody class="bg-white divide-y divide-gray-200">
            {% for job in jobs %}
            <tr id="job-{{ job.id }}">
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">
                    <code class="bg-gray-100 px-2 py-1 rounded">{{ job.id }}</code>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.job_type }}</td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.failed_at }}</td>
                <td class="px-6 py-4 text-sm text-red-600">
                    <details>
                        <summary class="cursor-pointer hover:text-red-900">{{ job.error_summary }}</summary>
                        <pre class="mt-2 p-2 bg-gray-50 rounded text-xs">{{ job.error_detail }}</pre>
                    </details>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.retry_count }}/{{ job.max_retries }}</td>
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium space-x-3">
                    <button hx-post="/admin/jobs/{{ job.id }}/retry"
                            hx-target="closest tr"
                            hx-swap="outerHTML"
                            class="text-blue-600 hover:text-blue-900">
                        Retry
                    </button>
                    <button hx-delete="/admin/jobs/{{ job.id }}"
                            hx-confirm="Delete this job?"
                            hx-target="closest tr"
                            hx-swap="delete"
                            class="text-red-600 hover:text-red-900">
                        Delete
                    </button>
                </td>
            </tr>
            {% endfor %}
        </tbody>
    </table>
</div>
{% endif %}
"#;

/// Job history list partial
pub const JOB_HISTORY_LIST: &str = r##"<div class="bg-white shadow overflow-hidden sm:rounded-lg">
    <table class="min-w-full divide-y divide-gray-200">
        <thead class="bg-gray-50">
            <tr>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Job ID</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Type</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Status</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Started</th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Duration</th>
            </tr>
        </thead>
        <tbody class="bg-white divide-y divide-gray-200">
            {% for job in jobs %}
            <tr>
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">
                    <code class="bg-gray-100 px-2 py-1 rounded">{{ job.id }}</code>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.job_type }}</td>
                <td class="px-6 py-4 whitespace-nowrap">
                    {% if job.status == "Completed" %}
                    <span class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-green-100 text-green-800">
                        {{ job.status }}
                    </span>
                    {% elsif job.status == "Failed" %}
                    <span class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-red-100 text-red-800">
                        {{ job.status }}
                    </span>
                    {% else %}
                    <span class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-gray-100 text-gray-800">
                        {{ job.status }}
                    </span>
                    {% endif %}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.started_at }}</td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ job.duration }}</td>
            </tr>
            {% endfor %}
        </tbody>
    </table>

    <!-- Pagination -->
    <div class="bg-white px-4 py-3 border-t border-gray-200 sm:px-6">
        <div class="flex items-center justify-between">
            <div class="text-sm text-gray-700">
                Showing <span class="font-medium">{{ page_start }}</span> to <span class="font-medium">{{ page_end }}</span> of{' '}
                <span class="font-medium">{{ total_jobs }}</span> results
            </div>
            <div class="flex space-x-2">
                {% if has_prev %}
                <button hx-get="/admin/jobs/history?page={{ prev_page }}"
                        hx-target="#job-history"
                        class="px-3 py-1 border border-gray-300 rounded text-sm hover:bg-gray-50">
                    Previous
                </button>
                {% endif %}
                {% if has_next %}
                <button hx-get="/admin/jobs/history?page={{ next_page }}"
                        hx-target="#job-history"
                        class="px-3 py-1 border border-gray-300 rounded text-sm hover:bg-gray-50">
                    Next
                </button>
                {% endif %}
            </div>
        </div>
    </div>
</div>
"##;

/// Job type scaffold template
pub const JOB_TEMPLATE: &str = r#"//! {{job_name}} background job

use acton_htmx::jobs::{Job, JobResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
{{#if needs_db}}
use sqlx::PgPool;
{{/if}}
{{#if needs_email}}
use acton_htmx::email::{Email, EmailSender};
{{/if}}

/// {{job_description}}
///
/// This job demonstrates the standard structure for background job implementation.
/// Customize the execute() method below to implement your specific business logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {{job_name}}Job {
{{#each fields}}
    {{#if this.doc}}
    /// {{this.doc}}
    {{/if}}
    pub {{this.name}}: {{this.rust_type}},
{{/each}}
}

#[async_trait]
impl Job for {{job_name}}Job {
    type Result = {{result_type}};

    async fn execute(&self) -> JobResult<Self::Result> {
        tracing::info!(
            job_type = "{{job_name}}",
{{#each fields}}
            {{this.name}} = ?self.{{this.name}},
{{/each}}
            "Executing {{job_name}} job"
        );

        // Implement your job logic here
        // Example patterns:
        //
        // 1. Email sending:
        //    let email = Email::new()
        //        .to(&self.recipient_email)
        //        .from("noreply@example.com")
        //        .subject("Subject")
        //        .text("Body");
        //    context.email_sender().send(email).await?;
        //
        // 2. Database operations:
        //    sqlx::query!("INSERT INTO table (col) VALUES ($1)", self.value)
        //        .execute(context.db_pool())
        //        .await?;
        //
        // 3. File processing:
        //    let content = context.file_storage()
        //        .retrieve(&self.file_id)
        //        .await?;
        //    // Process content...
        //
        // 4. External API calls:
        //    let response = reqwest::get(&self.api_url).await?;
        //    let data = response.json::<Data>().await?;

        Ok({{result_default}})
    }

    fn max_retries(&self) -> u32 {
        {{max_retries}}
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs({{timeout_secs}})
    }

    fn priority(&self) -> u8 {
        {{priority}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_{{job_name_snake}}_execution() {
        let job = {{job_name}}Job {
{{#each fields}}
            {{this.name}}: {{this.test_value}},
{{/each}}
        };

        let result = job.execute().await;
        assert!(result.is_ok());
    }
}
"#;

/// Job handler template for web admin
pub const JOB_HANDLER_TEMPLATE: &str = r#"//! Job administration handlers

use acton_htmx::{
    extractors::Authenticated,
    htmx::{HxRedirect, HxTrigger},
    jobs::{JobAgent, JobId, JobStatus},
    template::HxTemplate,
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{error::AppResult, models::User, AppState};

/// Job dashboard page
#[derive(Template)]
#[template(path = "jobs/dashboard.html")]
struct JobDashboardTemplate {
    running_count: usize,
    pending_count: usize,
    failed_count: usize,
}

pub async fn dashboard(
    _auth: Authenticated<User>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let metrics = state.jobs.metrics().await?;

    let template = JobDashboardTemplate {
        running_count: metrics.running,
        pending_count: metrics.pending,
        failed_count: metrics.failed,
    };

    Ok(template.into_response())
}

/// Job statistics partial
#[derive(Template)]
#[template(path = "jobs/_stats.html")]
struct JobStatsTemplate {
    total_jobs: u64,
    running_jobs: usize,
    completed_jobs: u64,
    failed_jobs: usize,
}

pub async fn stats(
    _auth: Authenticated<User>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let metrics = state.jobs.metrics().await?;

    let template = JobStatsTemplate {
        total_jobs: metrics.total_enqueued,
        running_jobs: metrics.running,
        completed_jobs: metrics.completed,
        failed_jobs: metrics.failed,
    };

    Ok(template.into_response())
}

/// Retry a failed job
pub async fn retry_job(
    _auth: Authenticated<User>,
    State(state): State<AppState>,
    Path(job_id): Path<JobId>,
) -> AppResult<Response> {
    state.jobs.retry_job(job_id).await?;

    Ok((
        HxTrigger::new("job-retried"),
        HxRedirect::to("/admin/jobs"),
    )
        .into_response())
}

/// Cancel a running job
pub async fn cancel_job(
    _auth: Authenticated<User>,
    State(state): State<AppState>,
    Path(job_id): Path<JobId>,
) -> AppResult<Response> {
    state.jobs.cancel_job(job_id).await?;

    Ok(HxTrigger::new("job-cancelled").into_response())
}

/// Clear dead letter queue
pub async fn clear_dead_letter(
    _auth: Authenticated<User>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    state.jobs.clear_dead_letter_queue().await?;

    Ok((
        HxTrigger::new("dlq-cleared"),
        HxRedirect::to("/admin/jobs"),
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default)]
    search: Option<String>,
}

fn default_page() -> usize {
    1
}

/// Job history with pagination and search
#[derive(Template)]
#[template(path = "jobs/_history_list.html")]
struct JobHistoryTemplate {
    jobs: Vec<JobHistoryItem>,
    page_start: usize,
    page_end: usize,
    total_jobs: usize,
    has_prev: bool,
    has_next: bool,
    prev_page: usize,
    next_page: usize,
}

/// Simplified job history item for template rendering
#[derive(Debug, Clone)]
struct JobHistoryItem {
    id: String,
    job_type: String,
    status: String,
    started_at: String,
    duration: String,
}

pub async fn history(
    _auth: Authenticated<User>,
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> AppResult<Response> {
    use acton_htmx::jobs::agent::GetJobHistoryRequest;
    use std::time::Duration;

    // Create request for job history
    let (request, rx) = GetJobHistoryRequest::new(
        query.page,
        20, // Page size
        query.search.clone(),
    );

    // Send request to JobAgent
    state.jobs.send(request).await;

    // Wait for response with timeout
    let timeout = Duration::from_millis(200);
    let history_page = tokio::time::timeout(timeout, rx)
        .await
        .map_err(|_| anyhow::anyhow!("Job history request timed out"))??;

    // Convert records to template items
    let jobs: Vec<JobHistoryItem> = history_page
        .jobs
        .into_iter()
        .map(|record| {
            let status = format!("{:?}", record.status);
            let started_at = record.started_at.format("%Y-%m-%d %H:%M:%S").to_string();
            let duration = if record.duration_ms < 1000 {
                format!("{}ms", record.duration_ms)
            } else {
                format!("{:.2}s", record.duration_ms as f64 / 1000.0)
            };

            JobHistoryItem {
                id: record.id.to_string(),
                job_type: record.job_type,
                status,
                started_at,
                duration,
            }
        })
        .collect();

    let template = JobHistoryTemplate {
        jobs,
        page_start: history_page.page_start(),
        page_end: history_page.page_end(),
        total_jobs: history_page.total_count,
        has_prev: history_page.has_prev,
        has_next: history_page.has_next,
        prev_page: history_page.prev_page(),
        next_page: history_page.next_page(),
    };

    Ok(template.into_response())
}
"#;

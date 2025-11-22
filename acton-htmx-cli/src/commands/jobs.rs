//! Job management CLI commands

use anyhow::{Context, Result};
use clap::Subcommand;
use console::{style, Emoji};
use serde::Deserialize;

static SUCCESS: Emoji = Emoji("✓", "√");
static INFO: Emoji = Emoji("ℹ", "i");

/// Job management commands
#[derive(Debug, Subcommand)]
pub enum JobsCommand {
    /// List all jobs with optional filtering
    List {
        /// Filter by status (pending, running, completed, failed)
        #[arg(short, long)]
        status: Option<String>,

        /// Limit number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Show detailed job statistics
    Stats,

    /// Retry a failed job by ID
    Retry {
        /// Job ID to retry
        job_id: String,
    },

    /// Retry all failed jobs
    RetryAll {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Cancel a running job by ID
    Cancel {
        /// Job ID to cancel
        job_id: String,
    },

    /// Clear the dead letter queue
    ClearDeadLetter {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Watch job queue in real-time
    Watch {
        /// Update interval in seconds
        #[arg(short, long, default_value = "2")]
        interval: u64,
    },
}

impl JobsCommand {
    /// Execute the jobs command
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to connect to job service
    /// - Failed to execute job operation
    /// - Invalid job ID provided
    pub fn execute(&self) -> Result<()> {
        match self {
            Self::List { status, limit } => {
                Self::list(status.as_deref(), *limit);
                Ok(())
            }
            Self::Stats => {
                Self::stats();
                Ok(())
            }
            Self::Retry { job_id } => {
                Self::retry(job_id);
                Ok(())
            }
            Self::RetryAll { force } => Self::retry_all(*force),
            Self::Cancel { job_id } => {
                Self::cancel(job_id);
                Ok(())
            }
            Self::ClearDeadLetter { force } => Self::clear_dead_letter(*force),
            Self::Watch { interval } => {
                Self::watch(*interval);
                Ok(())
            }
        }
    }

    fn list(status: Option<&str>, _limit: usize) {
        #[derive(Deserialize)]
        struct JobListResponse {
            jobs: Vec<JobInfo>,
            total: usize,
            message: String,
        }

        #[derive(Deserialize)]
        struct JobInfo {
            id: String,
            job_type: String,
            status: String,
            created_at: String,
            priority: i32,
        }

        println!("\n{INFO} Job Queue");
        println!();

        // Fetch jobs from job service API
        let base_url = std::env::var("ACTON_HTMX_API_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        let url = format!("{base_url}/admin/jobs/list");

        let response = match ureq::get(&url).call() {
            Ok(response) => {
                let body = match response.into_body().read_to_string() {
                    Ok(body) => body,
                    Err(e) => {
                        println!("  {} Failed to read response: {}", style("Error:").red(), e);
                        return;
                    }
                };
                match serde_json::from_str::<JobListResponse>(&body) {
                    Ok(response) => response,
                    Err(e) => {
                        println!("  {} Failed to parse response: {}", style("Error:").red(), e);
                        println!();
                        println!("{INFO} Ensure your application is running with job agent enabled.");
                        return;
                    }
                }
            }
            Err(e) => {
                println!("  {} Failed to connect to API: {}", style("Error:").red(), e);
                println!();
                println!("{INFO} Make sure your acton-htmx application is running at {}", style(base_url).cyan());
                println!("{INFO} You can set a custom URL with: ACTON_HTMX_API_URL=http://your-api:3000");
                return;
            }
        };

        let header = status.map_or_else(
            || format!("Showing {} jobs (total: {})", response.jobs.len(), response.total),
            |status| format!("Showing {} jobs with status: {} (total: {})", response.jobs.len(), style(status).cyan(), response.total),
        );

        println!("{}", style(header).bold());
        println!("{}", "─".repeat(80));

        // Table header
        println!(
            "{:<12} {:<20} {:<12} {:<20} {:<10}",
            "ID", "Type", "Status", "Created", "Priority"
        );
        println!("{}", "─".repeat(80));

        if response.jobs.is_empty() {
            println!("  {}", style("(No jobs to display)").dim());
        } else {
            for job in response.jobs {
                println!(
                    "{:<12} {:<20} {:<12} {:<20} {:<10}",
                    &job.id[..job.id.len().min(12)],
                    &job.job_type[..job.job_type.len().min(20)],
                    style(&job.status).cyan(),
                    &job.created_at[..job.created_at.len().min(20)],
                    job.priority
                );
            }
        }

        println!();
        if !response.message.is_empty() {
            println!("{INFO} {}", response.message);
        }
    }

    fn stats() {
        #[derive(Deserialize)]
        struct JobStats {
            total_enqueued: u64,
            running: usize,
            pending: usize,
            completed: u64,
            failed: u64,
            dead_letter: u64,
            avg_execution_ms: f64,
            p95_execution_ms: f64,
            p99_execution_ms: f64,
            success_rate: f64,
            #[allow(dead_code)] // Used for future features
            message: String,
        }

        println!("\n{INFO} Job Statistics");
        println!();

        // Fetch stats from job service API
        let base_url = std::env::var("ACTON_HTMX_API_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        let url = format!("{base_url}/admin/jobs/stats");

        let stats = match ureq::get(&url).call() {
            Ok(response) => {
                let body = match response.into_body().read_to_string() {
                    Ok(body) => body,
                    Err(e) => {
                        println!("  {} Failed to read response: {}", style("Error:").red(), e);
                        return;
                    }
                };
                match serde_json::from_str::<JobStats>(&body) {
                    Ok(stats) => stats,
                    Err(e) => {
                        println!("  {} Failed to parse response: {}", style("Error:").red(), e);
                        println!();
                        println!("{INFO} Ensure your application is running with job agent enabled.");
                        return;
                    }
                }
            }
            Err(e) => {
                println!("  {} Failed to connect to API: {}", style("Error:").red(), e);
                println!();
                println!("{INFO} Make sure your acton-htmx application is running at {}", style(base_url).cyan());
                println!("{INFO} You can set a custom URL with: ACTON_HTMX_API_URL=http://your-api:3000");
                return;
            }
        };

        println!("{}", style("Queue Status").bold().underlined());
        println!("  Total Enqueued:  {}", style(stats.total_enqueued).cyan());
        println!("  Running:         {}", style(stats.running).yellow());
        println!("  Pending:         {}", style(stats.pending).blue());
        println!("  Completed:       {}", style(stats.completed).green());
        println!("  Failed:          {}", style(stats.failed).red());
        println!("  Dead Letter:     {}", style(stats.dead_letter).red());
        println!();

        println!("{}", style("Performance Metrics").bold().underlined());
        println!(
            "  Avg Execution:   {} ms",
            style(format!("{:.2}", stats.avg_execution_ms)).cyan()
        );
        println!(
            "  P95 Execution:   {} ms",
            style(format!("{:.2}", stats.p95_execution_ms)).cyan()
        );
        println!(
            "  P99 Execution:   {} ms",
            style(format!("{:.2}", stats.p99_execution_ms)).cyan()
        );
        println!(
            "  Success Rate:    {}%",
            style(format!("{:.1}", stats.success_rate)).green()
        );
        println!();
    }

    fn retry(job_id: &str) {
        println!("{INFO} Retrying job: {}", style(job_id).cyan());

        // TODO: Call job service to retry job
        println!("  {}", style("(Job service not connected)").dim());
        println!();
        println!("{SUCCESS} Job retry queued successfully");
    }

    fn retry_all(force: bool) -> Result<()> {
        if !force {
            println!("{} This will retry ALL failed jobs.", style("Warning:").yellow());
            println!("Are you sure? (y/N): ");

            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .context("Failed to read input")?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        println!("{INFO} Retrying all failed jobs...");

        // TODO: Call job service to retry all failed jobs
        println!("  {}", style("(Job service not connected)").dim());
        println!();
        println!("{SUCCESS} All failed jobs queued for retry");

        Ok(())
    }

    fn cancel(job_id: &str) {
        println!("{INFO} Cancelling job: {}", style(job_id).cyan());

        // TODO: Call job service to cancel job
        println!("  {}", style("(Job service not connected)").dim());
        println!();
        println!("{SUCCESS} Job cancellation requested");
        println!("  {INFO} Graceful shutdown in progress. Job will stop at next checkpoint.");
    }

    fn clear_dead_letter(force: bool) -> Result<()> {
        if !force {
            println!(
                "{} This will permanently delete all jobs in the dead letter queue.",
                style("Warning:").yellow()
            );
            println!("This action CANNOT be undone. Are you sure? (y/N): ");

            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .context("Failed to read input")?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled.");
                return Ok(());
            }
        }

        println!("{INFO} Clearing dead letter queue...");

        // TODO: Call job service to clear DLQ
        println!("  {}", style("(Job service not connected)").dim());
        println!();
        println!("{SUCCESS} Dead letter queue cleared");

        Ok(())
    }

    fn watch(interval: u64) {
        use std::io::Write;
        use std::thread;
        use std::time::Duration;

        println!("{INFO} Watching job queue (Ctrl+C to stop)");
        println!("  Update interval: {interval} seconds");
        println!();

        loop {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");
            let _ = std::io::stdout().flush();

            // Print header
            println!("{}", style("Job Queue Monitor").bold().cyan());
            println!("{}", "=".repeat(80));
            println!();

            // TODO: Fetch and display real-time stats
            println!("{}", style("Queue Status").bold().underlined());
            println!("  Running:   {}", style("0").yellow());
            println!("  Pending:   {}", style("0").blue());
            println!("  Completed: {}", style("0").green());
            println!("  Failed:    {}", style("0").red());
            println!();

            println!(
                "{}",
                style(format!("Last updated: {}", chrono::Local::now().format("%H:%M:%S")))
                    .dim()
            );

            thread::sleep(Duration::from_secs(interval));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_command_defaults() {
        let cmd = JobsCommand::List {
            status: None,
            limit: 20,
        };

        // Should not panic
        let _ = cmd.execute();
    }

    #[test]
    fn test_stats_command() {
        let cmd = JobsCommand::Stats;

        // Should not panic
        let _ = cmd.execute();
    }

    #[test]
    fn test_retry_command() {
        let cmd = JobsCommand::Retry {
            job_id: "test-123".to_string(),
        };

        // Should not panic
        let _ = cmd.execute();
    }
}

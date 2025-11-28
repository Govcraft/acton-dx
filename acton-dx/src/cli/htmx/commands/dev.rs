//! Development server command with service integration
//!
//! This command starts the development server with optional service management:
//! - Hot reload via bacon (if installed)
//! - Embedded services mode for single-process development
//! - Service status display
//! - Environment variable configuration for services

use anyhow::{Context, Result};
use console::{style, Emoji};
use std::path::Path;
use std::process::{Command, Stdio};

use super::services::ServiceName;

static INFO: Emoji<'_, '_> = Emoji("ℹ", "i");
static RUNNING: Emoji<'_, '_> = Emoji("🟢", "[+]");
static STOPPED: Emoji<'_, '_> = Emoji("🔴", "[-]");

/// Development server options
#[derive(Debug, Clone)]
pub struct DevOptions {
    /// Run with embedded services (single process mode)
    pub embedded_services: bool,
    /// Services to enable (None means all services)
    pub services: Option<Vec<String>>,
    /// Base port for services (default: 50051)
    pub services_port: u16,
    /// Application port (default: 3000)
    pub app_port: u16,
    /// Host to bind to
    pub host: String,
}

impl Default for DevOptions {
    fn default() -> Self {
        Self {
            embedded_services: false,
            services: None,
            services_port: 50051,
            app_port: 3000,
            host: "127.0.0.1".to_string(),
        }
    }
}

/// Start development server with hot reload
pub struct DevCommand;

impl DevCommand {
    /// Create a new command instance
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Execute the command in the specified directory with default options
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The project directory doesn't exist or is not a valid project
    /// - The development server fails to start
    pub fn execute(path: &Path) -> Result<()> {
        Self::execute_with_options(path, &DevOptions::default())
    }

    /// Execute the command with custom options
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The project directory doesn't exist or is not a valid project
    /// - The development server fails to start
    pub fn execute_with_options(path: &Path, options: &DevOptions) -> Result<()> {
        // Canonicalize the path to get absolute path and verify it exists
        let project_dir = path
            .canonicalize()
            .with_context(|| format!("Project directory not found: {}", path.display()))?;

        // Verify Cargo.toml exists in the target directory
        if !project_dir.join("Cargo.toml").exists() {
            anyhow::bail!(
                "No Cargo.toml found in {}. Is this an Acton HTMX project?",
                project_dir.display()
            );
        }

        println!(
            "{} {} in {}",
            style("Starting").green().bold(),
            style("development server").bold(),
            style(project_dir.display()).cyan()
        );
        println!();

        // Show service status
        Self::show_service_status(options);

        // Check if bacon is installed
        if !Self::is_bacon_installed() {
            println!(
                "{} is not installed.",
                style("bacon").yellow().bold()
            );
            println!();
            println!("Install it with:");
            println!(
                "  {} {}",
                style("$").dim(),
                style("cargo install bacon").cyan()
            );
            println!();
            println!("For now, starting without hot reload...");
            println!();

            return Self::run_without_watch(&project_dir, options);
        }

        // Run with bacon for hot reload
        Self::run_with_bacon(&project_dir, options)
    }

    /// Show current service status
    fn show_service_status(options: &DevOptions) {
        println!("{INFO} Service Configuration:");
        println!();

        if options.embedded_services {
            println!(
                "  Mode:       {}",
                style("Embedded (single process)").yellow()
            );
        } else {
            println!(
                "  Mode:       {}",
                style("External services").dim()
            );
        }

        let enabled_services = options.services.as_ref().map_or_else(
            || "all".to_string(),
            |s| s.join(", "),
        );
        println!("  Services:   {}", style(&enabled_services).cyan());

        if options.embedded_services {
            println!(
                "  Base port:  {}",
                style(options.services_port.to_string()).cyan()
            );
        }
        println!(
            "  App port:   {}",
            style(options.app_port.to_string()).cyan()
        );
        println!("  Host:       {}", style(&options.host).cyan());
        println!();

        // Check external service status if not in embedded mode
        if !options.embedded_services {
            Self::check_external_services();
        }
    }

    /// Check status of external services
    fn check_external_services() {
        println!("{INFO} External Service Status:");
        println!();
        println!(
            "  {:<20} {:<10} {:<10}",
            "Service", "Status", "Port"
        );
        println!("  {}", "─".repeat(45));

        for service in ServiceName::all() {
            let port = service.default_port();
            let is_running = Self::is_port_in_use(port);
            let (emoji, status) = if is_running {
                (RUNNING, style("Running").green())
            } else {
                (STOPPED, style("Stopped").red())
            };

            println!(
                "  {:<20} {} {:<10} {:<10}",
                service.display_name(),
                emoji,
                status,
                port
            );
        }

        println!();

        // Check if any required services are not running
        let any_missing = ServiceName::all()
            .iter()
            .any(|s| !Self::is_port_in_use(s.default_port()));

        if any_missing {
            println!(
                "{INFO} {} Some services are not running. Start them with:",
                style("Note:").yellow()
            );
            println!(
                "       {} {}",
                style("$").dim(),
                style("acton-dx htmx services start auth data cedar").cyan()
            );
            println!();
            println!(
                "       Or use {} mode:",
                style("--embedded-services").yellow()
            );
            println!(
                "       {} {}",
                style("$").dim(),
                style("acton-dx htmx dev --embedded-services").cyan()
            );
            println!();
        }
    }

    /// Check if bacon is installed
    fn is_bacon_installed() -> bool {
        Command::new("bacon")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Check if a port is in use
    fn is_port_in_use(port: u16) -> bool {
        std::net::TcpListener::bind(format!("127.0.0.1:{port}")).is_err()
    }

    /// Build environment variables for the development server
    fn build_env_vars(options: &DevOptions) -> Vec<(&'static str, String)> {
        let mut env_vars = vec![
            ("ACTON_APP_PORT", options.app_port.to_string()),
            ("ACTON_APP_HOST", options.host.clone()),
        ];

        if options.embedded_services {
            env_vars.push(("ACTON_EMBEDDED_SERVICES", "true".to_string()));
            env_vars.push((
                "ACTON_SERVICES_BASE_PORT",
                options.services_port.to_string(),
            ));
        } else {
            env_vars.push(("ACTON_EMBEDDED_SERVICES", "false".to_string()));
        }

        if let Some(ref services) = options.services {
            env_vars.push(("ACTON_ENABLED_SERVICES", services.join(",")));
        }

        env_vars
    }

    /// Run with bacon for hot reload
    fn run_with_bacon(project_dir: &Path, options: &DevOptions) -> Result<()> {
        println!(
            "{}",
            style("Hot reload enabled via bacon. Watching for changes...").green()
        );
        println!();

        let mut cmd = Command::new("bacon");
        cmd.arg("run")
            .current_dir(project_dir);

        // Set environment variables
        for (key, value) in Self::build_env_vars(options) {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().context("Failed to start bacon")?;

        // Wait for the process to complete
        let status = child.wait().context("Failed to wait for bacon")?;

        if !status.success() {
            anyhow::bail!("Development server exited with error");
        }

        Ok(())
    }

    /// Run without hot reload (fallback)
    fn run_without_watch(project_dir: &Path, options: &DevOptions) -> Result<()> {
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .current_dir(project_dir);

        // Set environment variables
        for (key, value) in Self::build_env_vars(options) {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().context("Failed to start development server")?;

        let status = child
            .wait()
            .context("Failed to wait for development server")?;

        if !status.success() {
            anyhow::bail!("Development server exited with error");
        }

        Ok(())
    }
}

impl Default for DevCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let options = DevOptions::default();
        assert!(!options.embedded_services);
        assert!(options.services.is_none());
        assert_eq!(options.services_port, 50051);
        assert_eq!(options.app_port, 3000);
        assert_eq!(options.host, "127.0.0.1");
    }

    #[test]
    fn test_build_env_vars_standard() {
        let options = DevOptions::default();
        let vars = DevCommand::build_env_vars(&options);

        assert!(vars.iter().any(|(k, v)| *k == "ACTON_APP_PORT" && v == "3000"));
        assert!(vars.iter().any(|(k, v)| *k == "ACTON_APP_HOST" && v == "127.0.0.1"));
        assert!(vars.iter().any(|(k, v)| *k == "ACTON_EMBEDDED_SERVICES" && v == "false"));
    }

    #[test]
    fn test_build_env_vars_embedded() {
        let options = DevOptions {
            embedded_services: true,
            services: Some(vec!["auth".to_string(), "data".to_string()]),
            services_port: 60000,
            app_port: 8080,
            host: "0.0.0.0".to_string(),
        };
        let vars = DevCommand::build_env_vars(&options);

        assert!(vars.iter().any(|(k, v)| *k == "ACTON_APP_PORT" && v == "8080"));
        assert!(vars.iter().any(|(k, v)| *k == "ACTON_EMBEDDED_SERVICES" && v == "true"));
        assert!(vars.iter().any(|(k, v)| *k == "ACTON_SERVICES_BASE_PORT" && v == "60000"));
        assert!(vars.iter().any(|(k, v)| *k == "ACTON_ENABLED_SERVICES" && v == "auth,data"));
    }

    #[test]
    fn test_bacon_check_does_not_panic() {
        // Should not panic regardless of whether bacon is installed
        let _installed = DevCommand::is_bacon_installed();
    }
}

//! Serve command for running the application with optional embedded services.
//!
//! This command starts the application server with options for:
//! - Embedded services mode (all microservices in one process)
//! - Custom port configuration
//! - Service selection

use anyhow::{Context, Result};
use clap::Subcommand;
use console::{style, Emoji};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

static SUCCESS: Emoji<'_, '_> = Emoji("✓", "√");
static INFO: Emoji<'_, '_> = Emoji("ℹ", "i");
static STARTING: Emoji<'_, '_> = Emoji("🚀", ">>");

/// Serve command for running the application.
#[derive(Debug, Subcommand)]
pub enum ServeCommand {
    /// Start the application server
    Start {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Run with embedded services (single binary, no external services)
        #[arg(long)]
        embedded_services: bool,

        /// Base port for embedded services (default: 50051)
        #[arg(long, default_value = "50051")]
        services_port: u16,

        /// Application port (default: 3000)
        #[arg(long, short, default_value = "3000")]
        port: u16,

        /// Host to bind to (default: 127.0.0.1)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Run in release mode
        #[arg(long)]
        release: bool,

        /// Enable specific services only (comma-separated: auth,data,cedar,cache,email,file)
        #[arg(long, value_delimiter = ',')]
        services: Option<Vec<String>>,
    },
}

impl ServeCommand {
    /// Execute the serve command.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The project directory doesn't exist
    /// - The server fails to start
    /// - Service startup fails
    pub fn execute(&self) -> Result<()> {
        match self {
            Self::Start {
                path,
                embedded_services,
                services_port,
                port,
                host,
                release,
                services,
            } => Self::start(
                path,
                *embedded_services,
                *services_port,
                *port,
                host,
                *release,
                services.as_ref(),
            ),
        }
    }

    fn start(
        path: &Path,
        embedded_services: bool,
        services_port: u16,
        app_port: u16,
        host: &str,
        release: bool,
        services: Option<&Vec<String>>,
    ) -> Result<()> {
        // Canonicalize the path
        let project_dir = path
            .canonicalize()
            .with_context(|| format!("Project directory not found: {}", path.display()))?;

        // Verify Cargo.toml exists
        if !project_dir.join("Cargo.toml").exists() {
            anyhow::bail!(
                "No Cargo.toml found in {}. Is this an Acton HTMX project?",
                project_dir.display()
            );
        }

        println!();
        println!(
            "{STARTING} {} in {}",
            style("Starting application").green().bold(),
            style(project_dir.display()).cyan()
        );
        println!();

        if embedded_services {
            Self::start_embedded_mode(
                &project_dir,
                services_port,
                app_port,
                host,
                release,
                services,
            )
        } else {
            Self::start_standard_mode(&project_dir, app_port, host, release)
        }
    }

    fn start_embedded_mode(
        project_dir: &Path,
        services_port: u16,
        app_port: u16,
        host: &str,
        release: bool,
        services: Option<&Vec<String>>,
    ) -> Result<()> {
        println!(
            "{INFO} {}",
            style("Embedded services mode enabled").yellow()
        );
        println!();

        // List enabled services
        let enabled_services =
            services.map_or_else(|| "all".to_string(), |s| s.join(", "));

        println!("  Services:   {}", style(&enabled_services).cyan());
        println!(
            "  Base port:  {}",
            style(services_port.to_string()).cyan()
        );
        println!("  App port:   {}", style(app_port.to_string()).cyan());
        println!("  Host:       {}", style(host).cyan());
        println!();

        // Set environment variables for embedded services
        let mut env_vars = vec![
            ("ACTON_EMBEDDED_SERVICES", "true".to_string()),
            ("ACTON_SERVICES_BASE_PORT", services_port.to_string()),
            ("ACTON_APP_PORT", app_port.to_string()),
            ("ACTON_APP_HOST", host.to_string()),
        ];

        if let Some(svc) = services {
            env_vars.push(("ACTON_ENABLED_SERVICES", svc.join(",")));
        }

        Self::run_cargo(project_dir, release, &env_vars)
    }

    fn start_standard_mode(
        project_dir: &Path,
        app_port: u16,
        host: &str,
        release: bool,
    ) -> Result<()> {
        println!(
            "{INFO} {}",
            style("Standard mode (external services)").dim()
        );
        println!();
        println!("  App port:   {}", style(app_port.to_string()).cyan());
        println!("  Host:       {}", style(host).cyan());
        println!();

        let env_vars = vec![
            ("ACTON_EMBEDDED_SERVICES", "false".to_string()),
            ("ACTON_APP_PORT", app_port.to_string()),
            ("ACTON_APP_HOST", host.to_string()),
        ];

        Self::run_cargo(project_dir, release, &env_vars)
    }

    fn run_cargo(
        project_dir: &Path,
        release: bool,
        env_vars: &[(&str, String)],
    ) -> Result<()> {
        let mut cmd = Command::new("cargo");
        cmd.arg("run");

        if release {
            cmd.arg("--release");
            println!("{INFO} Building in release mode...");
        } else {
            println!("{INFO} Building in debug mode...");
        }

        println!();

        cmd.current_dir(project_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // Set environment variables
        for (key, value) in env_vars {
            cmd.env(*key, value);
        }

        let mut child = cmd.spawn().context("Failed to start application")?;

        let status = child.wait().context("Failed to wait for application")?;

        if !status.success() {
            anyhow::bail!("Application exited with error");
        }

        println!();
        println!("  {SUCCESS} Application stopped gracefully");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_command_parses() {
        // Verify command structure is valid
        let cmd = ServeCommand::Start {
            path: PathBuf::from("."),
            embedded_services: true,
            services_port: 50051,
            port: 3000,
            host: "127.0.0.1".to_string(),
            release: false,
            services: Some(vec!["auth".to_string(), "data".to_string()]),
        };

        match cmd {
            ServeCommand::Start {
                embedded_services, ..
            } => {
                assert!(embedded_services);
            }
        }
    }

    #[test]
    fn test_default_values() {
        let cmd = ServeCommand::Start {
            path: PathBuf::from("."),
            embedded_services: false,
            services_port: 50051,
            port: 3000,
            host: "127.0.0.1".to_string(),
            release: false,
            services: None,
        };

        match cmd {
            ServeCommand::Start {
                services_port,
                port,
                host,
                ..
            } => {
                assert_eq!(services_port, 50051);
                assert_eq!(port, 3000);
                assert_eq!(host, "127.0.0.1");
            }
        }
    }
}

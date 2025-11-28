//! Services management CLI commands
//!
//! Commands for managing microservices:
//! - `start` - Start one or more services
//! - `stop` - Stop one or more services
//! - `status` - Show status of all services
//! - `logs` - View service logs

use anyhow::{Context, Result};
use clap::Subcommand;
use console::{style, Emoji};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static SUCCESS: Emoji<'_, '_> = Emoji("✓", "√");
static ERROR: Emoji<'_, '_> = Emoji("✗", "x");
static INFO: Emoji<'_, '_> = Emoji("ℹ", "i");
static RUNNING: Emoji<'_, '_> = Emoji("🟢", "[+]");
static STOPPED: Emoji<'_, '_> = Emoji("🔴", "[-]");
static STARTING: Emoji<'_, '_> = Emoji("🟡", "[~]");

/// Available microservices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
pub enum ServiceName {
    /// Authentication service (sessions, passwords, CSRF)
    Auth,
    /// Data service (database queries, transactions)
    Data,
    /// Cedar authorization service
    Cedar,
    /// Cache service (Redis operations)
    Cache,
    /// Email service
    Email,
    /// File storage service
    File,
}

impl ServiceName {
    /// Get the binary name for this service
    #[must_use]
    pub const fn binary_name(&self) -> &'static str {
        match self {
            Self::Auth => "auth-service",
            Self::Data => "data-service",
            Self::Cedar => "cedar-service",
            Self::Cache => "cache-service",
            Self::Email => "email-service",
            Self::File => "file-service",
        }
    }

    /// Get the default port for this service
    #[must_use]
    pub const fn default_port(&self) -> u16 {
        match self {
            Self::Auth => 50051,
            Self::Data => 50052,
            Self::Cedar => 50053,
            Self::Cache => 50054,
            Self::Email => 50055,
            Self::File => 50056,
        }
    }

    /// Get the display name for this service
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Auth => "Auth Service",
            Self::Data => "Data Service",
            Self::Cedar => "Cedar Service",
            Self::Cache => "Cache Service",
            Self::Email => "Email Service",
            Self::File => "File Service",
        }
    }

    /// Get all service names
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Auth,
            Self::Data,
            Self::Cedar,
            Self::Cache,
            Self::Email,
            Self::File,
        ]
    }
}

impl std::fmt::Display for ServiceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Service process info
struct ServiceProcess {
    #[allow(dead_code)]
    name: ServiceName,
    child: Child,
    #[allow(dead_code)]
    port: u16,
}

/// Global service manager state
static SERVICE_MANAGER: std::sync::LazyLock<Arc<Mutex<HashMap<ServiceName, ServiceProcess>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Services management commands
#[derive(Debug, Subcommand)]
pub enum ServicesCommand {
    /// Start one or more microservices
    Start {
        /// Services to start (auth, data, cedar, cache, email, file)
        #[arg(required = true)]
        services: Vec<ServiceName>,

        /// Build services before starting (runs cargo build)
        #[arg(long)]
        build: bool,

        /// Run in foreground (don't daemonize)
        #[arg(long, short)]
        foreground: bool,
    },

    /// Stop one or more microservices
    Stop {
        /// Services to stop (or 'all' to stop all running services)
        services: Vec<ServiceName>,

        /// Stop all running services
        #[arg(long)]
        all: bool,
    },

    /// Show status of all microservices
    Status,

    /// View service logs
    Logs {
        /// Service to view logs for
        service: ServiceName,

        /// Follow log output
        #[arg(long, short)]
        follow: bool,

        /// Number of lines to show
        #[arg(long, short, default_value = "50")]
        lines: usize,
    },

    /// Restart one or more services
    Restart {
        /// Services to restart
        #[arg(required = true)]
        services: Vec<ServiceName>,
    },
}

impl ServicesCommand {
    /// Execute the services command
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to start/stop service processes
    /// - Failed to connect to services
    /// - Invalid service name provided
    pub fn execute(&self) -> Result<()> {
        match self {
            Self::Start {
                services,
                build,
                foreground,
            } => Self::start(services, *build, *foreground),
            Self::Stop { services, all } => Self::stop(services, *all),
            Self::Status => {
                Self::status();
                Ok(())
            }
            Self::Logs {
                service,
                follow,
                lines,
            } => Self::logs(*service, *follow, *lines),
            Self::Restart { services } => Self::restart(services),
        }
    }

    fn start(services: &[ServiceName], build: bool, foreground: bool) -> Result<()> {
        println!("\n{INFO} Starting services...");
        println!();

        // Build services if requested
        if build {
            println!("{STARTING} Building services...");
            let service_names: Vec<_> = services.iter().map(ServiceName::binary_name).collect();
            for name in &service_names {
                let status = std::process::Command::new("cargo")
                    .args(["build", "--release", "-p", name])
                    .status()
                    .context(format!("Failed to build {name}"))?;

                if !status.success() {
                    anyhow::bail!("Failed to build {name}");
                }
                println!("  {SUCCESS} Built {name}");
            }
            println!();
        }

        // Start each service
        for service in services {
            let binary = service.binary_name();
            let port = service.default_port();

            // Check if already running
            if Self::is_service_running(*service) {
                println!(
                    "  {INFO} {} already running on port {port}",
                    style(service.display_name()).cyan(),
                );
                continue;
            }

            println!(
                "{STARTING} Starting {} on port {port}...",
                style(service.display_name()).cyan(),
            );

            // Find the binary path
            let binary_path = Self::find_binary(binary)?;

            if foreground && services.len() == 1 {
                // Run in foreground (blocking)
                Self::run_foreground(&binary_path, port)?;
            } else {
                // Start as background process
                Self::start_background(*service, &binary_path, port)?;
            }
        }

        // Show status after starting
        if !foreground || services.len() > 1 {
            println!();
            Self::status();
        }

        Ok(())
    }

    fn stop(services: &[ServiceName], all: bool) -> Result<()> {
        println!("\n{INFO} Stopping services...");
        println!();

        let services_to_stop: Vec<ServiceName> = if all {
            ServiceName::all().to_vec()
        } else if services.is_empty() {
            anyhow::bail!("Specify services to stop or use --all");
        } else {
            services.to_vec()
        };

        for service in services_to_stop {
            if Self::stop_service(service) {
                println!(
                    "  {SUCCESS} Stopped {}",
                    style(service.display_name()).cyan()
                );
            } else {
                println!(
                    "  {INFO} {} was not running",
                    style(service.display_name()).dim()
                );
            }
        }

        println!();
        Ok(())
    }

    fn status() {
        println!("\n{INFO} Service Status");
        println!();
        println!(
            "{:<20} {:<10} {:<10} {:<15}",
            "Service", "Status", "Port", "PID"
        );
        println!("{}", "─".repeat(60));

        for service in ServiceName::all() {
            let port = service.default_port();
            let (status_emoji, status_text, pid) = if Self::is_service_running(*service) {
                let pid = Self::get_service_pid(*service)
                    .map_or_else(|| "?".to_string(), |p| p.to_string());
                (RUNNING, style("Running").green(), pid)
            } else {
                (STOPPED, style("Stopped").red(), "-".to_string())
            };

            println!(
                "{:<20} {} {:<10} {:<10} {:<15}",
                service.display_name(),
                status_emoji,
                status_text,
                port,
                pid
            );
        }

        println!();

        // Check if any services are running via ports
        Self::check_port_status();
    }

    fn logs(service: ServiceName, follow: bool, lines: usize) -> Result<()> {
        let log_dir = Self::get_log_dir();
        let log_file = log_dir.join(format!("{}.log", service.binary_name()));

        if !log_file.exists() {
            println!(
                "{ERROR} No logs found for {}",
                style(service.display_name()).cyan()
            );
            println!("{INFO} Service may not have been started yet.");
            return Ok(());
        }

        if follow {
            // Follow mode - use tail -f
            let mut cmd = std::process::Command::new("tail")
                .args(["-f", "-n", &lines.to_string()])
                .arg(&log_file)
                .spawn()
                .context("Failed to tail log file")?;

            // Wait for Ctrl+C
            cmd.wait()?;
        } else {
            // Show last N lines
            let file =
                std::fs::File::open(&log_file).context("Failed to open log file")?;
            let reader = BufReader::new(file);
            let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
            let start = all_lines.len().saturating_sub(lines);

            println!(
                "{INFO} Last {lines} lines from {}:",
                style(service.display_name()).cyan()
            );
            println!();

            for line in all_lines.iter().skip(start) {
                println!("{line}");
            }
        }

        Ok(())
    }

    fn restart(services: &[ServiceName]) -> Result<()> {
        println!("\n{INFO} Restarting services...");
        println!();

        for service in services {
            println!(
                "{STARTING} Restarting {}...",
                style(service.display_name()).cyan()
            );

            // Stop if running
            Self::stop_service(*service);

            // Small delay to allow port to be released
            std::thread::sleep(Duration::from_millis(500));

            // Start again
            let binary = service.binary_name();
            let port = service.default_port();
            let binary_path = Self::find_binary(binary)?;
            Self::start_background(*service, &binary_path, port)?;

            println!(
                "  {SUCCESS} Restarted {} on port {port}",
                style(service.display_name()).cyan(),
            );
        }

        println!();
        Self::status();
        Ok(())
    }

    // Helper functions

    fn find_binary(name: &str) -> Result<PathBuf> {
        // Check release build first
        let release_path = PathBuf::from(format!("target/release/{name}"));
        if release_path.exists() {
            return Ok(release_path);
        }

        // Check debug build
        let debug_path = PathBuf::from(format!("target/debug/{name}"));
        if debug_path.exists() {
            return Ok(debug_path);
        }

        // Try to find in PATH using which command
        if let Ok(output) = std::process::Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                return Ok(PathBuf::from(path));
            }
        }

        anyhow::bail!(
            "Service binary '{name}' not found. Run with --build to compile first.",
        )
    }

    fn run_foreground(binary_path: &PathBuf, port: u16) -> Result<()> {
        let status = std::process::Command::new(binary_path)
            .env("SERVICE_PORT", port.to_string())
            .status()
            .context("Failed to run service")?;

        if !status.success() {
            anyhow::bail!("Service exited with error");
        }

        Ok(())
    }

    fn start_background(
        service: ServiceName,
        binary_path: &PathBuf,
        port: u16,
    ) -> Result<()> {
        let log_dir = Self::get_log_dir();
        std::fs::create_dir_all(&log_dir)?;

        let log_file = log_dir.join(format!("{}.log", service.binary_name()));
        let log_handle = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .context("Failed to open log file")?;

        let child = std::process::Command::new(binary_path)
            .env("SERVICE_PORT", port.to_string())
            .stdout(Stdio::from(
                log_handle.try_clone().context("Failed to clone log handle")?,
            ))
            .stderr(Stdio::from(log_handle))
            .spawn()
            .context("Failed to start service")?;

        let pid = child.id();

        // Store in manager and immediately drop the lock
        {
            let mut manager = SERVICE_MANAGER.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
            manager.insert(
                service,
                ServiceProcess {
                    name: service,
                    child,
                    port,
                },
            );
        }

        // Write PID file
        let pid_file = Self::get_pid_file(service);
        std::fs::write(&pid_file, pid.to_string())?;

        println!(
            "  {SUCCESS} Started {} (PID: {pid}, Port: {port})",
            style(service.display_name()).cyan(),
        );

        Ok(())
    }

    fn stop_service(service: ServiceName) -> bool {
        // Try to stop via manager first
        let Ok(mut manager) = SERVICE_MANAGER.lock() else {
            return false;
        };

        if let Some(mut process) = manager.remove(&service) {
            drop(manager); // Release lock before waiting
            let _ = process.child.kill();
            let _ = process.child.wait();
            Self::cleanup_pid_file(service);
            return true;
        }

        drop(manager); // Release lock before file operations

        // Try to stop via PID file
        let pid_file = Self::get_pid_file(service);
        if pid_file.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    // Send termination signal
                    Self::terminate_process(pid);
                }
            }
            Self::cleanup_pid_file(service);
            return true;
        }

        false
    }

    #[cfg(unix)]
    fn terminate_process(pid: u32) {
        // Use kill command to send SIGTERM - safe approach without unsafe blocks
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }

    #[cfg(not(unix))]
    fn terminate_process(pid: u32) {
        // On Windows, use taskkill
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }

    fn is_service_running(service: ServiceName) -> bool {
        // Check manager first
        if let Ok(manager) = SERVICE_MANAGER.lock() {
            if manager.contains_key(&service) {
                return true;
            }
        }

        // Check PID file
        let pid_file = Self::get_pid_file(service);
        if pid_file.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    return Self::is_process_alive(pid);
                }
            }
        }

        // Check if port is in use
        Self::is_port_in_use(service.default_port())
    }

    #[cfg(unix)]
    fn is_process_alive(pid: u32) -> bool {
        // Use kill -0 to check if process exists - safe approach
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .is_ok_and(|o| o.status.success())
    }

    #[cfg(not(unix))]
    fn is_process_alive(pid: u32) -> bool {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
    }

    fn get_service_pid(service: ServiceName) -> Option<u32> {
        // Check manager
        if let Ok(manager) = SERVICE_MANAGER.lock() {
            if let Some(process) = manager.get(&service) {
                return Some(process.child.id());
            }
        }

        // Check PID file
        let pid_file = Self::get_pid_file(service);
        if pid_file.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
                return pid_str.trim().parse().ok();
            }
        }

        None
    }

    fn is_port_in_use(port: u16) -> bool {
        std::net::TcpListener::bind(format!("127.0.0.1:{port}")).is_err()
    }

    fn check_port_status() {
        let mut issues = Vec::new();

        for service in ServiceName::all() {
            let port = service.default_port();
            if Self::is_port_in_use(port) && !Self::is_service_running(*service) {
                issues.push(format!(
                    "Port {port} is in use but {} is not tracked (external process?)",
                    service.display_name()
                ));
            }
        }

        if !issues.is_empty() {
            println!("{}", style("Notes:").yellow().bold());
            for issue in issues {
                println!("  {INFO} {issue}");
            }
            println!();
        }
    }

    fn get_log_dir() -> PathBuf {
        let base = std::env::var("XDG_STATE_HOME").map_or_else(
            |_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/state")
            },
            PathBuf::from,
        );
        base.join("acton-dx/services/logs")
    }

    fn get_pid_file(service: ServiceName) -> PathBuf {
        let base = std::env::var("XDG_RUNTIME_DIR")
            .map_or_else(|_| std::env::temp_dir(), PathBuf::from);
        base.join(format!("acton-dx-{}.pid", service.binary_name()))
    }

    fn cleanup_pid_file(service: ServiceName) {
        let pid_file = Self::get_pid_file(service);
        let _ = std::fs::remove_file(pid_file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_name_binary() {
        assert_eq!(ServiceName::Auth.binary_name(), "auth-service");
        assert_eq!(ServiceName::Data.binary_name(), "data-service");
        assert_eq!(ServiceName::Cedar.binary_name(), "cedar-service");
    }

    #[test]
    fn test_service_name_ports() {
        assert_eq!(ServiceName::Auth.default_port(), 50051);
        assert_eq!(ServiceName::Data.default_port(), 50052);
        assert_eq!(ServiceName::Cedar.default_port(), 50053);
    }

    #[test]
    fn test_service_name_all() {
        let all = ServiceName::all();
        assert_eq!(all.len(), 6);
    }

    #[test]
    fn test_service_display() {
        assert_eq!(format!("{}", ServiceName::Auth), "Auth Service");
        assert_eq!(format!("{}", ServiceName::File), "File Service");
    }

    #[test]
    fn test_status_command_does_not_panic() {
        // Should not panic even without running services
        let _ = ServicesCommand::Status.execute();
    }

    #[test]
    fn test_log_dir_path() {
        let log_dir = ServicesCommand::get_log_dir();
        assert!(log_dir.ends_with("acton-dx/services/logs"));
    }

    #[test]
    fn test_pid_file_path() {
        let pid_file = ServicesCommand::get_pid_file(ServiceName::Auth);
        assert!(pid_file
            .file_name()
            .is_some_and(|name| name == "acton-dx-auth-service.pid"));
    }
}

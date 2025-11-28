//! HTMX CLI commands
//!
//! Commands for creating and managing HTMX web applications:
//! - `new` - Create new project
//! - `dev` - Start development server
//! - `db` - Database management
//! - `scaffold` - Generate CRUD resources
//! - `generate` - Generate code (jobs, deployment)
//! - `templates` - Manage framework templates
//! - `jobs` - Manage background jobs
//! - `deploy` - Deploy to production

pub mod commands;
pub mod project_template_manager;
pub mod scaffold;
pub mod static_templates;
pub mod template_manager;

use anyhow::Result;
use clap::Subcommand;
use commands::{
    DbCommand, DeployCommand, DevCommand, GenerateCommand, JobsCommand, NewCommand,
    OAuth2Command, ScaffoldCommand, ServicesCommand, TemplatesCommand,
};

pub use project_template_manager::ProjectTemplateManager;
pub use scaffold::{FieldDefinition, FieldType, ScaffoldGenerator, TemplateHelpers};
pub use template_manager::TemplateManager;

/// Database backend for new projects
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum DatabaseBackend {
    /// `SQLite` - zero setup, perfect for development (default)
    #[default]
    Sqlite,
    /// `PostgreSQL` - production-grade, requires installation
    Postgres,
}

/// HTMX subcommand
#[derive(Subcommand)]
pub enum HtmxCommand {
    /// Create a new acton-dx project
    New {
        /// Project name
        name: String,
        /// Database backend (sqlite or postgres)
        #[arg(short, long, default_value = "sqlite")]
        database: DatabaseBackend,
    },
    /// Start development server with hot reload
    Dev {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        path: std::path::PathBuf,
    },
    /// Database management commands
    Db {
        /// Database subcommand to execute
        #[command(subcommand)]
        command: DbCommands,
    },
    /// Generate CRUD scaffold
    Scaffold {
        /// Scaffold subcommand to execute
        #[command(subcommand)]
        command: ScaffoldCommands,
    },
    /// Generate code (jobs, models, etc.)
    Generate {
        /// Generate subcommand to execute
        #[command(subcommand)]
        command: GenerateCommand,
    },
    /// Manage background jobs
    Jobs {
        /// Jobs subcommand to execute
        #[command(subcommand)]
        command: JobsCommand,
    },
    /// Deploy to production
    Deploy {
        /// Deploy subcommand to execute
        #[command(subcommand)]
        command: DeployCommand,
    },
    /// Check application health
    HealthCheck {
        /// Health check URL (default: <http://localhost:8080/health>)
        #[arg(long, default_value = "http://localhost:8080/health")]
        url: String,
    },
    /// Manage framework templates
    Templates {
        /// Templates subcommand to execute
        #[command(subcommand)]
        command: TemplatesCommand,
    },
    /// Manage microservices
    Services {
        /// Services subcommand to execute
        #[command(subcommand)]
        command: ServicesCommand,
    },
}

/// Scaffold subcommands
#[derive(Subcommand)]
pub enum ScaffoldCommands {
    /// Generate complete CRUD resource
    Crud {
        /// Model name (`PascalCase`, e.g., `Post`, `UserProfile`)
        model: String,
        /// Field definitions (e.g., `title:string`, `author:references:User`)
        #[arg(required = true)]
        fields: Vec<String>,
    },
    /// Set up `OAuth2` authentication for a provider
    OAuth2 {
        /// Provider name (google, github, oidc)
        provider: String,
    },
}

/// Database subcommands
#[derive(Subcommand)]
pub enum DbCommands {
    /// Run pending migrations
    Migrate,
    /// Reset database (drop, create, migrate)
    Reset,
    /// Create new migration
    Create {
        /// Migration name
        name: String,
    },
}

/// Run an HTMX CLI command
///
/// # Errors
///
/// Returns an error if the command fails to execute
pub fn run(command: HtmxCommand) -> Result<()> {
    match command {
        HtmxCommand::New { name, database } => {
            let cmd = NewCommand::new(name, database)?;
            cmd.execute()?;
        }
        HtmxCommand::Dev { path } => {
            DevCommand::execute(&path)?;
        }
        HtmxCommand::Db { command } => {
            let db_cmd = match command {
                DbCommands::Migrate => DbCommand::Migrate,
                DbCommands::Reset => DbCommand::Reset,
                DbCommands::Create { name } => DbCommand::Create { name },
            };
            db_cmd.execute()?;
        }
        HtmxCommand::Scaffold { command } => match command {
            ScaffoldCommands::Crud { model, fields } => {
                let cmd = ScaffoldCommand::new(model, fields);
                cmd.execute()?;
            }
            ScaffoldCommands::OAuth2 { provider } => {
                let cmd = OAuth2Command::new(provider);
                cmd.execute()?;
            }
        },
        HtmxCommand::Generate { command } => {
            command.execute()?;
        }
        HtmxCommand::Jobs { command } => {
            command.execute()?;
        }
        HtmxCommand::Deploy { command } => {
            command.execute()?;
        }
        HtmxCommand::HealthCheck { url } => {
            health_check(&url)?;
        }
        HtmxCommand::Templates { command } => {
            command.execute()?;
        }
        HtmxCommand::Services { command } => {
            command.execute()?;
        }
    }

    Ok(())
}

/// Check application health
fn health_check(url: &str) -> Result<()> {
    use console::{style, Emoji};

    static CHECKING: Emoji<'_, '_> = Emoji("🔍", ">>>");
    static SUCCESS: Emoji<'_, '_> = Emoji("✓", "√");
    static ERROR: Emoji<'_, '_> = Emoji("✗", "x");

    println!(
        "{} Checking application health at: {}",
        CHECKING,
        style(url).cyan()
    );
    println!();

    // Make HTTP request (ureq 3.x call() returns Result with timeout handling)
    let response = ureq::get(url).call();

    match response {
        Ok(mut resp) => {
            let status = resp.status();
            let body = resp
                .body_mut()
                .read_to_string()
                .unwrap_or_else(|_| "Could not read response".to_string());

            if status == 200 {
                println!("  {SUCCESS} Application is healthy (HTTP {status})");
                println!();
                println!("{}", style("Response:").bold());
                println!("{body}");
                println!();
                Ok(())
            } else {
                println!("  {ERROR} Application health check failed (HTTP {status})");
                println!();
                println!("{}", style("Response:").bold());
                println!("{body}");
                println!();
                anyhow::bail!("Health check returned status: {status}");
            }
        }
        Err(e) => {
            println!("  {ERROR} Health check failed: {e}");
            println!();
            println!("Possible issues:");
            println!("  - Application is not running");
            println!("  - Wrong URL (check host and port)");
            println!("  - Health endpoint not configured");
            println!();
            anyhow::bail!("Could not reach health endpoint");
        }
    }
}

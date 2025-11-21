//! acton-htmx CLI tool

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::multiple_crate_versions)]

mod commands;
pub mod templates;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::{DbCommand, DevCommand, NewCommand, ScaffoldCommand};

// Re-export for library usage
pub use templates::ProjectTemplate;

#[derive(Parser)]
#[command(name = "acton-htmx")]
#[command(version)]
#[command(about = "CLI tool for acton-htmx framework", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new acton-htmx project
    New {
        /// Project name
        name: String,
    },
    /// Start development server with hot reload
    Dev,
    /// Database management commands
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
    /// Generate CRUD scaffold
    Scaffold {
        #[command(subcommand)]
        command: ScaffoldCommands,
    },
}

#[derive(Subcommand)]
enum ScaffoldCommands {
    /// Generate complete CRUD resource
    Crud {
        /// Model name (`PascalCase`, e.g., `Post`, `UserProfile`)
        model: String,
        /// Field definitions (e.g., `title:string`, `author:references:User`)
        #[arg(required = true)]
        fields: Vec<String>,
    },
}

#[derive(Subcommand)]
enum DbCommands {
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => {
            let cmd = NewCommand::new(name)?;
            cmd.execute()?;
        }
        Commands::Dev => {
            DevCommand::execute()?;
        }
        Commands::Db { command } => {
            let db_cmd = match command {
                DbCommands::Migrate => DbCommand::Migrate,
                DbCommands::Reset => DbCommand::Reset,
                DbCommands::Create { name } => DbCommand::Create { name },
            };
            db_cmd.execute()?;
        }
        Commands::Scaffold { command } => {
            match command {
                ScaffoldCommands::Crud { model, fields } => {
                    let cmd = ScaffoldCommand::new(model, fields);
                    cmd.execute()?;
                }
            }
        }
    }

    Ok(())
}

//! CRUD scaffold generator for acton-htmx
//!
//! This module provides intelligent code generation for complete CRUD resources.
//! It generates:
//! - `SeaORM` models with validation
//! - Database migrations
//! - Form structs with validation
//! - HTMX handlers (list, show, new, edit, delete) - coming soon
//! - Askama templates - coming soon
//! - Integration tests - coming soon
//! - Route registration - coming soon
//!
//! # Example
//!
//! ```bash
//! acton-htmx scaffold crud Post \
//!   title:string \
//!   content:text \
//!   author:references:User \
//!   published:boolean \
//!   published_at:datetime:optional
//! ```

use acton_htmx::scaffold::generator::ScaffoldGenerator;
use acton_htmx::scaffold::helpers::TemplateHelpers;
use anyhow::{Context, Result};
use console::style;
use std::fs;

pub struct ScaffoldCommand {
    model: String,
    fields: Vec<String>,
}

impl ScaffoldCommand {
    pub const fn new(model: String, fields: Vec<String>) -> Self {
        Self { model, fields }
    }

    pub fn execute(&self) -> Result<()> {
        println!(
            "\n{} {} {}",
            style("Scaffolding CRUD for").cyan().bold(),
            style(&self.model).green().bold(),
            style("...").cyan().bold()
        );

        // Get current directory as project root
        let project_root = std::env::current_dir()
            .context("Failed to get current directory")?;

        // Create generator
        let generator = ScaffoldGenerator::new(
            self.model.clone(),
            &self.fields,
            project_root.clone(),
        )
        .context("Failed to create scaffold generator")?;

        // Generate files
        let files = generator.generate()
            .context("Failed to generate scaffold files")?;

        println!(
            "\n{} {} files:",
            style("Generated").green().bold(),
            files.len()
        );

        // Write files to disk
        for file in &files {
            let full_path = project_root.join(&file.path);

            // Create parent directories if they don't exist
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }

            // Write file
            fs::write(&full_path, &file.content)
                .with_context(|| format!("Failed to write file: {}", full_path.display()))?;

            println!(
                "  {} {} ({})",
                style("✓").green(),
                style(file.path.display()).dim(),
                style(&file.description).dim()
            );
        }

        println!(
            "\n{} CRUD scaffold for {} is ready!",
            style("✨").green().bold(),
            style(&self.model).green().bold()
        );

        println!("\n{}", style("Next steps:").cyan().bold());
        println!("  1. Run the migration: {}", style("cargo run --bin migrate").yellow());
        println!("  2. Add model to src/models/mod.rs: {}", style(format!("pub mod {};", TemplateHelpers::to_snake_case(&self.model))).yellow());
        println!("  3. Add form to src/forms/mod.rs: {}", style(format!("pub mod {};", TemplateHelpers::to_snake_case(&self.model))).yellow());
        println!("  4. Build your project: {}", style("cargo build").yellow());

        Ok(())
    }
}

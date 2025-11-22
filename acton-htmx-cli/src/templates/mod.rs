//! Project template generation

use anyhow::{Context, Result};
use handlebars::Handlebars;
use serde_json::json;
use std::fs;
use std::path::Path;

pub mod deployment;
pub mod files;
pub mod jobs;
pub use deployment::*;
pub use files::*;
pub use jobs::JOB_TEMPLATE;

/// Project template generator
pub struct ProjectTemplate {
    name: String,
    handlebars: Handlebars<'static>,
}

impl ProjectTemplate {
    /// Create a new project template
    pub fn new(name: &str) -> Self {
        let mut handlebars = Handlebars::new();

        // Disable HTML escaping since we're generating code
        handlebars.register_escape_fn(handlebars::no_escape);

        Self {
            name: name.to_string(),
            handlebars,
        }
    }

    /// Generate all project files
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Template rendering fails
    /// - File writing fails
    /// - Directory creation fails
    pub fn generate(&self, output_dir: &Path) -> Result<()> {
        let context = json!({
            "project_name": self.name,
            "project_name_snake": self.name.replace('-', "_"),
        });

        // Generate each file
        self.write_file(output_dir, "Cargo.toml", CARGO_TOML, &context)?;
        self.write_file(output_dir, "README.md", README_MD, &context)?;
        self.write_file(output_dir, ".gitignore", GITIGNORE, &context)?;
        self.write_file(output_dir, "config/development.toml", CONFIG_DEV, &context)?;
        self.write_file(output_dir, "config/production.toml", CONFIG_PROD, &context)?;
        self.write_file(output_dir, "src/main.rs", MAIN_RS, &context)?;
        self.write_file(output_dir, "src/handlers/mod.rs", HANDLERS_MOD, &context)?;
        self.write_file(output_dir, "src/handlers/home.rs", HANDLERS_HOME, &context)?;
        self.write_file(output_dir, "src/handlers/auth.rs", HANDLERS_AUTH, &context)?;
        self.write_file(output_dir, "src/models/mod.rs", MODELS_MOD, &context)?;
        self.write_file(output_dir, "src/models/user.rs", MODELS_USER, &context)?;
        self.write_file(output_dir, "templates/layouts/base.html", TEMPLATE_BASE, &context)?;
        self.write_file(output_dir, "templates/layouts/app.html", TEMPLATE_APP, &context)?;
        self.write_file(output_dir, "templates/auth/login.html", TEMPLATE_LOGIN, &context)?;
        self.write_file(output_dir, "templates/auth/register.html", TEMPLATE_REGISTER, &context)?;
        self.write_file(output_dir, "templates/partials/flash.html", TEMPLATE_FLASH, &context)?;
        self.write_file(output_dir, "templates/partials/nav.html", TEMPLATE_NAV, &context)?;
        self.write_file(output_dir, "templates/home.html", TEMPLATE_HOME, &context)?;
        self.write_file(output_dir, "static/css/app.css", STATIC_CSS, &context)?;
        self.write_file(output_dir, "migrations/001_create_users.sql", MIGRATION_USERS, &context)?;

        Ok(())
    }

    /// Write a single file from template
    fn write_file(
        &self,
        output_dir: &Path,
        relative_path: &str,
        template: &str,
        context: &serde_json::Value,
    ) -> Result<()> {
        let path = output_dir.join(relative_path);

        let rendered = self
            .handlebars
            .render_template(template, context)
            .with_context(|| format!("Failed to render template: {relative_path}"))?;

        fs::write(&path, rendered)
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        Ok(())
    }
}

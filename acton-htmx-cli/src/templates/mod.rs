//! Project template generation

use anyhow::{Context, Result};
use handlebars::Handlebars;
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::DatabaseBackend;

pub mod deployment;
pub mod files;
pub mod jobs;
pub use deployment::*;
pub use files::*;
pub use jobs::JOB_TEMPLATE;

/// Project template generator
pub struct ProjectTemplate {
    name: String,
    database: DatabaseBackend,
    handlebars: Handlebars<'static>,
}

impl ProjectTemplate {
    /// Create a new project template
    pub fn new(name: &str, database: DatabaseBackend) -> Self {
        let mut handlebars = Handlebars::new();

        // Disable HTML escaping since we're generating code
        handlebars.register_escape_fn(handlebars::no_escape);

        Self {
            name: name.to_string(),
            database,
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
        let is_sqlite = matches!(self.database, DatabaseBackend::Sqlite);

        let context = json!({
            "project_name": self.name,
            "project_name_snake": self.name.replace('-', "_"),
            "database": if is_sqlite { "sqlite" } else { "postgres" },
            "is_sqlite": is_sqlite,
            "is_postgres": !is_sqlite,
        });

        // Generate Cargo.toml based on database backend
        let cargo_toml = if is_sqlite { CARGO_TOML_SQLITE } else { CARGO_TOML_POSTGRES };
        self.write_file(output_dir, "Cargo.toml", cargo_toml, &context)?;

        // Generate README based on database backend
        let readme = if is_sqlite { README_MD_SQLITE } else { README_MD_POSTGRES };
        self.write_file(output_dir, "README.md", readme, &context)?;

        self.write_file(output_dir, ".gitignore", GITIGNORE, &context)?;

        // Generate config based on database backend
        let config_dev = if is_sqlite { CONFIG_DEV_SQLITE } else { CONFIG_DEV_POSTGRES };
        let config_prod = if is_sqlite { CONFIG_PROD_SQLITE } else { CONFIG_PROD_POSTGRES };
        self.write_file(output_dir, "config/development.toml", config_dev, &context)?;
        self.write_file(output_dir, "config/production.toml", config_prod, &context)?;

        // Generate main.rs based on database backend
        let main_rs = if is_sqlite { MAIN_RS_SQLITE } else { MAIN_RS_POSTGRES };
        self.write_file(output_dir, "src/main.rs", main_rs, &context)?;

        self.write_file(output_dir, "src/handlers/mod.rs", HANDLERS_MOD, &context)?;
        self.write_file(output_dir, "src/handlers/home.rs", HANDLERS_HOME, &context)?;

        // Generate auth handler based on database backend
        let handlers_auth = if is_sqlite { HANDLERS_AUTH_SQLITE } else { HANDLERS_AUTH_POSTGRES };
        self.write_file(output_dir, "src/handlers/auth.rs", handlers_auth, &context)?;

        self.write_file(output_dir, "src/models/mod.rs", MODELS_MOD, &context)?;
        self.write_file(output_dir, "templates/layouts/base.html", TEMPLATE_BASE, &context)?;
        self.write_file(output_dir, "templates/layouts/app.html", TEMPLATE_APP, &context)?;
        self.write_file(output_dir, "templates/auth/login.html", TEMPLATE_LOGIN, &context)?;
        self.write_file(output_dir, "templates/auth/register.html", TEMPLATE_REGISTER, &context)?;
        self.write_file(output_dir, "templates/partials/flash.html", TEMPLATE_FLASH, &context)?;
        self.write_file(output_dir, "templates/partials/nav.html", TEMPLATE_NAV, &context)?;
        self.write_file(output_dir, "templates/home.html", TEMPLATE_HOME, &context)?;
        self.write_file(output_dir, "static/css/app.css", STATIC_CSS, &context)?;

        // Generate migration based on database backend
        let migration = if is_sqlite { MIGRATION_USERS_SQLITE } else { MIGRATION_USERS_POSTGRES };
        self.write_file(output_dir, "migrations/001_create_users.sql", migration, &context)?;

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

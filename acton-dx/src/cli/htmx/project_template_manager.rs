//! Project template manager for `acton htmx new` command
//!
//! Manages downloading and caching project templates from GitHub with XDG compliance.
//! Templates can be customized by users by placing modified versions in the XDG config directory.
//!
//! Uses minijinja for template rendering, which is Jinja2-compatible and consistent
//! with Askama templates used in generated projects.

use anyhow::{Context, Result};
use minijinja::Environment;
use std::fs;
use std::path::{Path, PathBuf};

use super::DatabaseBackend;

/// GitHub raw content base URL for project templates
const GITHUB_RAW_BASE: &str = "https://raw.githubusercontent.com/Govcraft/acton-dx/main/templates/project";

/// Common template files shared between SQLite and PostgreSQL
const COMMON_TEMPLATES: &[&str] = &[
    "common/src/handlers/mod.rs.hbs",
    "common/src/handlers/home.rs.hbs",
    "common/src/models/mod.rs.hbs",
    "common/templates/layouts/base.html.hbs",
    "common/templates/layouts/app.html.hbs",
    "common/templates/partials/nav.html.hbs",
    "common/templates/partials/flash.html.hbs",
    "common/templates/partials/auth_success.html.hbs",
    "common/templates/partials/logout_success.html.hbs",
    "common/templates/partials/home_content.html.hbs",
    "common/templates/partials/login_content.html.hbs",
    "common/templates/auth/login.html.hbs",
    "common/templates/auth/register.html.hbs",
    "common/templates/home.html.hbs",
    "common/static/css/app.css.hbs",
    "common/static/favicon.ico",
    "common/.gitignore.hbs",
];

/// SQLite-specific template files
const SQLITE_TEMPLATES: &[&str] = &[
    "sqlite/Cargo.toml.hbs",
    "sqlite/README.md.hbs",
    "sqlite/src/main.rs.hbs",
    "sqlite/src/handlers/auth.rs.hbs",
    "sqlite/config/development.toml.hbs",
    "sqlite/config/production.toml.hbs",
    "sqlite/migrations/001_create_users.sql.hbs",
];

/// PostgreSQL-specific template files
const POSTGRES_TEMPLATES: &[&str] = &[
    "postgres/Cargo.toml.hbs",
    "postgres/README.md.hbs",
    "postgres/src/main.rs.hbs",
    "postgres/src/handlers/auth.rs.hbs",
    "postgres/config/development.toml.hbs",
    "postgres/config/production.toml.hbs",
    "postgres/migrations/001_create_users.sql.hbs",
];

/// Mapping from template source path to output path
struct TemplateMapping {
    /// Path in template directory (relative to database backend dir)
    source: &'static str,
    /// Output path in generated project
    output: &'static str,
}

/// Common template mappings (remove "common/" prefix)
const COMMON_MAPPINGS: &[TemplateMapping] = &[
    TemplateMapping { source: "common/src/handlers/mod.rs.hbs", output: "src/handlers/mod.rs" },
    TemplateMapping { source: "common/src/handlers/home.rs.hbs", output: "src/handlers/home.rs" },
    TemplateMapping { source: "common/src/models/mod.rs.hbs", output: "src/models/mod.rs" },
    TemplateMapping { source: "common/templates/layouts/base.html.hbs", output: "templates/layouts/base.html" },
    TemplateMapping { source: "common/templates/layouts/app.html.hbs", output: "templates/layouts/app.html" },
    TemplateMapping { source: "common/templates/partials/nav.html.hbs", output: "templates/partials/nav.html" },
    TemplateMapping { source: "common/templates/partials/flash.html.hbs", output: "templates/partials/flash.html" },
    TemplateMapping { source: "common/templates/partials/auth_success.html.hbs", output: "templates/partials/auth_success.html" },
    TemplateMapping { source: "common/templates/partials/logout_success.html.hbs", output: "templates/partials/logout_success.html" },
    TemplateMapping { source: "common/templates/partials/home_content.html.hbs", output: "templates/partials/home_content.html" },
    TemplateMapping { source: "common/templates/partials/login_content.html.hbs", output: "templates/partials/login_content.html" },
    TemplateMapping { source: "common/templates/auth/login.html.hbs", output: "templates/auth/login.html" },
    TemplateMapping { source: "common/templates/auth/register.html.hbs", output: "templates/auth/register.html" },
    TemplateMapping { source: "common/templates/home.html.hbs", output: "templates/home.html" },
    TemplateMapping { source: "common/static/css/app.css.hbs", output: "static/css/app.css" },
    TemplateMapping { source: "common/static/favicon.ico", output: "static/favicon.ico" },
    TemplateMapping { source: "common/.gitignore.hbs", output: ".gitignore" },
];

/// SQLite template mappings (remove "sqlite/" prefix)
const SQLITE_MAPPINGS: &[TemplateMapping] = &[
    TemplateMapping { source: "sqlite/Cargo.toml.hbs", output: "Cargo.toml" },
    TemplateMapping { source: "sqlite/README.md.hbs", output: "README.md" },
    TemplateMapping { source: "sqlite/src/main.rs.hbs", output: "src/main.rs" },
    TemplateMapping { source: "sqlite/src/handlers/auth.rs.hbs", output: "src/handlers/auth.rs" },
    TemplateMapping { source: "sqlite/config/development.toml.hbs", output: "config/development.toml" },
    TemplateMapping { source: "sqlite/config/production.toml.hbs", output: "config/production.toml" },
    TemplateMapping { source: "sqlite/migrations/001_create_users.sql.hbs", output: "migrations/001_create_users.sql" },
];

/// PostgreSQL template mappings (remove "postgres/" prefix)
const POSTGRES_MAPPINGS: &[TemplateMapping] = &[
    TemplateMapping { source: "postgres/Cargo.toml.hbs", output: "Cargo.toml" },
    TemplateMapping { source: "postgres/README.md.hbs", output: "README.md" },
    TemplateMapping { source: "postgres/src/main.rs.hbs", output: "src/main.rs" },
    TemplateMapping { source: "postgres/src/handlers/auth.rs.hbs", output: "src/handlers/auth.rs" },
    TemplateMapping { source: "postgres/config/development.toml.hbs", output: "config/development.toml" },
    TemplateMapping { source: "postgres/config/production.toml.hbs", output: "config/production.toml" },
    TemplateMapping { source: "postgres/migrations/001_create_users.sql.hbs", output: "migrations/001_create_users.sql" },
];

/// Project template manager for downloading and generating new projects
pub struct ProjectTemplateManager {
    /// Local repo templates path (for development)
    repo_templates: Option<PathBuf>,
    /// XDG config path for user customizations
    user_config: PathBuf,
    /// XDG cache path for downloaded templates
    cache: PathBuf,
}

impl ProjectTemplateManager {
    /// Create a new project template manager
    ///
    /// # Errors
    ///
    /// Returns an error if directories cannot be created.
    pub fn new() -> Result<Self> {
        let repo_templates = Self::find_repo_templates_dir();
        let user_config = Self::get_config_dir()?;
        let cache = Self::get_cache_dir()?;

        fs::create_dir_all(&user_config)
            .with_context(|| format!("Failed to create config directory: {}", user_config.display()))?;
        fs::create_dir_all(&cache)
            .with_context(|| format!("Failed to create cache directory: {}", cache.display()))?;

        Ok(Self {
            repo_templates,
            user_config,
            cache,
        })
    }

    /// Find templates directory in local repo (for development)
    fn find_repo_templates_dir() -> Option<PathBuf> {
        // Try to find templates relative to the executable
        if let Ok(exe_path) = std::env::current_exe() {
            // Check if running from cargo target directory
            if let Some(target_dir) = exe_path.parent() {
                // Go up from target/debug to workspace root
                let workspace_root = target_dir
                    .parent() // debug
                    .and_then(|p| p.parent()); // target

                if let Some(root) = workspace_root {
                    let templates_dir = root.join("templates").join("project");
                    if templates_dir.exists() {
                        return Some(templates_dir);
                    }
                }
            }
        }

        // Also check CARGO_MANIFEST_DIR for development
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let workspace_root = PathBuf::from(manifest_dir)
                .parent()
                .map(Path::to_path_buf);

            if let Some(root) = workspace_root {
                let templates_dir = root.join("templates").join("project");
                if templates_dir.exists() {
                    return Some(templates_dir);
                }
            }
        }

        None
    }

    /// Get XDG config directory for user-customized templates
    fn get_config_dir() -> Result<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg)
        } else {
            let home = std::env::var("HOME").context("HOME not set")?;
            PathBuf::from(home).join(".config")
        };
        Ok(base.join("acton-dx").join("templates").join("project"))
    }

    /// Get XDG cache directory for downloaded templates
    fn get_cache_dir() -> Result<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            PathBuf::from(xdg)
        } else {
            let home = std::env::var("HOME").context("HOME not set")?;
            PathBuf::from(home).join(".cache")
        };
        Ok(base.join("acton-dx").join("templates").join("project"))
    }

    /// Ensure all templates for the given database backend are available
    ///
    /// # Errors
    ///
    /// Returns an error if templates cannot be downloaded.
    pub fn ensure_templates(&self, database: DatabaseBackend) -> Result<()> {
        let all_templates = Self::get_templates_for_backend(database);
        let missing = self.get_missing_templates(&all_templates);

        if missing.is_empty() {
            return Ok(());
        }

        println!("Downloading {} project templates from GitHub...", missing.len());

        for template_name in missing {
            self.download_template(&template_name)
                .with_context(|| format!("Failed to download template: {template_name}"))?;
        }

        println!("✓ Templates downloaded");
        Ok(())
    }

    /// Get all template names for a database backend
    fn get_templates_for_backend(database: DatabaseBackend) -> Vec<&'static str> {
        let mut templates: Vec<&'static str> = COMMON_TEMPLATES.to_vec();
        match database {
            DatabaseBackend::Sqlite => templates.extend(SQLITE_TEMPLATES),
            DatabaseBackend::Postgres => templates.extend(POSTGRES_TEMPLATES),
        }
        templates
    }

    /// Get templates that are not yet cached
    fn get_missing_templates(&self, templates: &[&str]) -> Vec<String> {
        templates
            .iter()
            .filter(|&&name| {
                // Check repo dir (development), then config dir, then cache dir
                let in_repo = self.repo_templates.as_ref().is_some_and(|d| d.join(name).exists());
                let in_config = self.user_config.join(name).exists();
                let in_cache = self.cache.join(name).exists();
                !in_repo && !in_config && !in_cache
            })
            .map(|&name| name.to_string())
            .collect()
    }

    /// Download a single template from GitHub
    fn download_template(&self, name: &str) -> Result<()> {
        let url = format!("{GITHUB_RAW_BASE}/{name}");
        let response = ureq::get(&url)
            .call()
            .with_context(|| format!("Failed to fetch template from {url}"))?;

        let mut body = response.into_body();
        let content = body.read_to_vec().context("Failed to read template content")?;

        let dest_path = self.cache.join(name);

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        fs::write(&dest_path, content)
            .with_context(|| format!("Failed to write template: {}", dest_path.display()))?;

        println!("  ✓ {name}");
        Ok(())
    }

    /// Resolve template path with priority: repo (dev) > config (user) > cache (downloaded)
    fn resolve_template(&self, name: &str) -> Result<PathBuf> {
        // Check repo dir first (development mode)
        if let Some(ref repo_dir) = self.repo_templates {
            let repo_path = repo_dir.join(name);
            if repo_path.exists() {
                return Ok(repo_path);
            }
        }

        // Check user config (customizations)
        let config_path = self.user_config.join(name);
        if config_path.exists() {
            return Ok(config_path);
        }

        // Check cache (downloaded defaults)
        let cache_path = self.cache.join(name);
        if cache_path.exists() {
            return Ok(cache_path);
        }

        anyhow::bail!("Template not found: {name}. Run `acton htmx templates update` to download.")
    }

    /// Generate a new project from templates
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Templates cannot be resolved
    /// - Template rendering fails
    /// - File writing fails
    pub fn generate_project(
        &self,
        project_name: &str,
        output_dir: &Path,
        database: DatabaseBackend,
    ) -> Result<()> {
        // Create minijinja environment
        let mut env = Environment::new();

        // Disable auto-escaping since we're generating code, not HTML
        env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

        // Create template context
        let context = minijinja::context! {
            project_name => project_name,
            project_name_snake => project_name.replace('-', "_"),
            database => match database {
                DatabaseBackend::Sqlite => "sqlite",
                DatabaseBackend::Postgres => "postgres",
            },
            is_sqlite => matches!(database, DatabaseBackend::Sqlite),
            is_postgres => matches!(database, DatabaseBackend::Postgres),
        };

        // Get mappings for this database backend
        let db_mappings = match database {
            DatabaseBackend::Sqlite => SQLITE_MAPPINGS,
            DatabaseBackend::Postgres => POSTGRES_MAPPINGS,
        };

        // Process common templates
        for mapping in COMMON_MAPPINGS {
            self.process_template(&env, mapping, output_dir, &context)?;
        }

        // Process database-specific templates
        for mapping in db_mappings {
            self.process_template(&env, mapping, output_dir, &context)?;
        }

        Ok(())
    }

    /// Check if a file is binary based on extension
    fn is_binary_file(path: &str) -> bool {
        let binary_extensions = [".ico", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".woff", ".woff2", ".ttf", ".eot"];
        binary_extensions.iter().any(|ext| path.ends_with(ext))
    }

    /// Process a single template mapping
    fn process_template(
        &self,
        env: &Environment<'_>,
        mapping: &TemplateMapping,
        output_dir: &Path,
        context: &minijinja::Value,
    ) -> Result<()> {
        let template_path = self.resolve_template(mapping.source)?;
        let output_path = output_dir.join(mapping.output);

        // Create output directory
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Handle binary files by copying directly
        if Self::is_binary_file(mapping.source) {
            fs::copy(&template_path, &output_path)
                .with_context(|| format!("Failed to copy binary file: {}", template_path.display()))?;
            return Ok(());
        }

        // Read template content
        let template_content = fs::read_to_string(&template_path)
            .with_context(|| format!("Failed to read template: {}", template_path.display()))?;

        // Check if this is an Askama template (contains {% extends or {% block)
        // These should be copied as-is since they'll be processed by Askama in the generated project
        let is_askama_template = template_content.contains("{% extends")
            || template_content.contains("{%- extends")
            || (mapping.output.contains("templates/")
                && std::path::Path::new(mapping.output)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("html")));

        let output_content = if is_askama_template {
            // Copy Askama templates verbatim, only substitute simple variables
            // using basic string replacement (not Jinja processing)
            let project_name = context
                .get_attr("project_name")
                .ok()
                .map(|v| v.to_string())
                .unwrap_or_default();
            template_content
                .replace("{{project_name}}", &project_name)
                .replace("{{ project_name }}", &project_name)
        } else {
            // Render non-Askama templates using minijinja
            env.render_str(&template_content, context)
                .with_context(|| format!("Failed to render template: {}", mapping.source))?
        };

        // Write output
        fs::write(&output_path, output_content)
            .with_context(|| format!("Failed to write file: {}", output_path.display()))?;

        Ok(())
    }

    /// Force re-download all templates
    ///
    /// # Errors
    ///
    /// Returns an error if templates cannot be downloaded.
    pub fn update_templates(&self) -> Result<()> {
        println!("Updating project templates from GitHub...");

        // Download all templates (both SQLite and PostgreSQL)
        let mut all_templates: Vec<&str> = COMMON_TEMPLATES.to_vec();
        all_templates.extend(SQLITE_TEMPLATES);
        all_templates.extend(POSTGRES_TEMPLATES);

        for template in all_templates {
            self.download_template(template)?;
        }

        println!("✓ All project templates updated");
        Ok(())
    }
}

impl Default for ProjectTemplateManager {
    fn default() -> Self {
        Self::new().expect("Failed to create project template manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir_uses_xdg() {
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/test-xdg-config");
        let dir = ProjectTemplateManager::get_config_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/test-xdg-config/acton-dx/templates/project"));
    }

    #[test]
    fn test_cache_dir_uses_xdg() {
        std::env::set_var("XDG_CACHE_HOME", "/tmp/test-xdg-cache");
        let dir = ProjectTemplateManager::get_cache_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/test-xdg-cache/acton-dx/templates/project"));
    }

    #[test]
    fn test_common_templates_list() {
        assert!(COMMON_TEMPLATES.len() >= 10);
        assert!(COMMON_TEMPLATES.contains(&"common/src/handlers/mod.rs.hbs"));
    }

    #[test]
    fn test_sqlite_templates_list() {
        assert!(SQLITE_TEMPLATES.len() >= 5);
        assert!(SQLITE_TEMPLATES.contains(&"sqlite/src/main.rs.hbs"));
    }

    #[test]
    fn test_postgres_templates_list() {
        assert!(POSTGRES_TEMPLATES.len() >= 5);
        assert!(POSTGRES_TEMPLATES.contains(&"postgres/src/main.rs.hbs"));
    }
}

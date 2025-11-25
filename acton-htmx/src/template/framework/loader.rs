//! Framework template loader with XDG resolution and hot reload support

use minijinja::{Environment, Value};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

use super::TEMPLATE_NAMES;

/// Errors that can occur when loading or rendering framework templates
#[derive(Debug, Error)]
pub enum FrameworkTemplateError {
    /// Template file could not be read
    #[error("failed to read template '{0}': {1}")]
    ReadFailed(String, std::io::Error),

    /// Template was not found in any location
    #[error("template not found: {0}")]
    NotFound(String),

    /// Template rendering failed
    #[error("template render error: {0}")]
    RenderError(#[from] minijinja::Error),

    /// XDG directory resolution failed
    #[error("failed to resolve XDG directory: {0}")]
    XdgError(String),

    /// Templates not initialized - user needs to run CLI
    #[error(
        "Framework templates not found.\n\n\
        Templates must exist in one of these locations:\n\
        - {config_dir}\n\
        - {cache_dir}\n\n\
        To initialize templates, run:\n\
        \x1b[1m  acton-htmx templates init\x1b[0m\n\n\
        Or download manually from:\n\
        \x1b[4mhttps://github.com/Govcraft/acton-htmx/tree/main/templates/framework\x1b[0m"
    )]
    TemplatesNotInitialized {
        /// Config directory path
        config_dir: String,
        /// Cache directory path
        cache_dir: String,
    },
}

/// Thread-safe framework template environment with hot reload support
///
/// Templates are loaded from XDG directories with embedded fallbacks.
/// The environment supports atomic reload for development hot-reload.
#[derive(Debug)]
pub struct FrameworkTemplates {
    env: Arc<RwLock<Environment<'static>>>,
    config_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
}

impl FrameworkTemplates {
    /// Create a new framework templates instance
    ///
    /// Loads templates from XDG directories. Templates MUST exist on disk
    /// (either in config or cache directory). Run `acton-htmx templates init`
    /// to download them.
    ///
    /// # Errors
    ///
    /// Returns error if templates are not found or cannot be loaded.
    pub fn new() -> Result<Self, FrameworkTemplateError> {
        let config_dir = Self::get_config_dir();
        let cache_dir = Self::get_cache_dir();

        // Verify templates exist before loading
        Self::verify_templates_exist(config_dir.as_ref(), cache_dir.as_ref())?;

        let env = Self::create_environment(config_dir.as_ref(), cache_dir.as_ref())?;

        Ok(Self {
            env: Arc::new(RwLock::new(env)),
            config_dir,
            cache_dir,
        })
    }

    /// Verify that templates exist in at least one XDG location
    fn verify_templates_exist(
        config_dir: Option<&PathBuf>,
        cache_dir: Option<&PathBuf>,
    ) -> Result<(), FrameworkTemplateError> {
        // Check if at least one required template exists
        let test_template = "forms/form.html";

        let config_exists = config_dir.is_some_and(|d| d.join(test_template).exists());

        let cache_exists = cache_dir.is_some_and(|d| d.join(test_template).exists());

        if !config_exists && !cache_exists {
            return Err(FrameworkTemplateError::TemplatesNotInitialized {
                config_dir: config_dir.map_or_else(
                    || "~/.config/acton-htmx/templates/framework".to_string(),
                    |p| p.display().to_string(),
                ),
                cache_dir: cache_dir.map_or_else(
                    || "~/.cache/acton-htmx/templates/framework".to_string(),
                    |p| p.display().to_string(),
                ),
            });
        }

        Ok(())
    }

    /// Get the XDG config directory for framework templates
    ///
    /// Returns `$XDG_CONFIG_HOME/acton-htmx/templates/framework/` or
    /// `~/.config/acton-htmx/templates/framework/` if not set.
    #[must_use]
    pub fn get_config_dir() -> Option<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg)
        } else {
            dirs::home_dir()?.join(".config")
        };
        Some(base.join("acton-htmx").join("templates").join("framework"))
    }

    /// Get the XDG cache directory for framework templates
    ///
    /// Returns `$XDG_CACHE_HOME/acton-htmx/templates/framework/` or
    /// `~/.cache/acton-htmx/templates/framework/` if not set.
    #[must_use]
    pub fn get_cache_dir() -> Option<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            PathBuf::from(xdg)
        } else {
            dirs::home_dir()?.join(".cache")
        };
        Some(base.join("acton-htmx").join("templates").join("framework"))
    }

    /// Create a new minijinja environment with all templates loaded
    fn create_environment(
        config_dir: Option<&PathBuf>,
        cache_dir: Option<&PathBuf>,
    ) -> Result<Environment<'static>, FrameworkTemplateError> {
        let mut env = Environment::new();

        // Configure environment
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);

        // Load all templates
        for name in TEMPLATE_NAMES {
            let content = Self::load_template_content(name, config_dir, cache_dir)?;
            env.add_template_owned((*name).to_string(), content)?;
        }

        Ok(env)
    }

    /// Load template content with XDG resolution order
    ///
    /// Templates are loaded from disk only - no embedded fallback.
    /// Order: config (customizations) > cache (defaults)
    fn load_template_content(
        name: &str,
        config_dir: Option<&PathBuf>,
        cache_dir: Option<&PathBuf>,
    ) -> Result<String, FrameworkTemplateError> {
        // 1. Check user config (customized templates)
        if let Some(dir) = config_dir {
            let path = dir.join(name);
            if path.exists() {
                return std::fs::read_to_string(&path)
                    .map_err(|e| FrameworkTemplateError::ReadFailed(name.to_string(), e));
            }
        }

        // 2. Check cache (downloaded defaults)
        if let Some(dir) = cache_dir {
            let path = dir.join(name);
            if path.exists() {
                return std::fs::read_to_string(&path)
                    .map_err(|e| FrameworkTemplateError::ReadFailed(name.to_string(), e));
            }
        }

        // No embedded fallback - templates must be on disk
        Err(FrameworkTemplateError::NotFound(name.to_string()))
    }

    /// Get embedded template content (for CLI to write to cache)
    ///
    /// This is used by the CLI `templates init` command to populate the cache.
    #[must_use]
    pub fn get_embedded_template(name: &str) -> Option<&'static str> {
        EMBEDDED_TEMPLATES.get(name).copied()
    }

    /// Get all embedded template names
    pub fn embedded_template_names() -> impl Iterator<Item = &'static str> {
        EMBEDDED_TEMPLATES.keys().copied()
    }

    /// Render a template with the given context
    ///
    /// # Errors
    ///
    /// Returns error if the template is not found or rendering fails.
    pub fn render(&self, name: &str, ctx: Value) -> Result<String, FrameworkTemplateError> {
        self.env
            .read()
            .get_template(name)
            .and_then(|tmpl| tmpl.render(ctx))
            .map_err(Into::into)
    }

    /// Render a template with a context map
    ///
    /// Convenience method that accepts a HashMap instead of minijinja::Value.
    ///
    /// # Errors
    ///
    /// Returns error if the template is not found or rendering fails.
    pub fn render_with_map(
        &self,
        name: &str,
        ctx: HashMap<&str, Value>,
    ) -> Result<String, FrameworkTemplateError> {
        self.env
            .read()
            .get_template(name)
            .and_then(|tmpl| tmpl.render(ctx))
            .map_err(Into::into)
    }

    /// Reload all templates from disk
    ///
    /// Useful for hot-reload during development. Creates a new environment
    /// and atomically swaps it with the current one.
    ///
    /// # Errors
    ///
    /// Returns error if templates cannot be reloaded.
    pub fn reload(&self) -> Result<(), FrameworkTemplateError> {
        let new_env =
            Self::create_environment(self.config_dir.as_ref(), self.cache_dir.as_ref())?;

        // Atomic swap
        *self.env.write() = new_env;

        tracing::debug!("Framework templates reloaded");
        Ok(())
    }

    /// Check if a template exists in user config (customized)
    #[must_use]
    pub fn is_customized(&self, name: &str) -> bool {
        self.config_dir
            .as_ref()
            .is_some_and(|dir| dir.join(name).exists())
    }

    /// Get the path where a template would be loaded from
    ///
    /// Returns the actual file path if found on disk, or None if using embedded.
    #[must_use]
    pub fn get_template_path(&self, name: &str) -> Option<PathBuf> {
        // Check config dir first
        if let Some(dir) = &self.config_dir {
            let path = dir.join(name);
            if path.exists() {
                return Some(path);
            }
        }

        // Check cache dir
        if let Some(dir) = &self.cache_dir {
            let path = dir.join(name);
            if path.exists() {
                return Some(path);
            }
        }

        // Using embedded
        None
    }

    /// Get a reference to the config directory
    #[must_use]
    pub const fn config_dir(&self) -> Option<&PathBuf> {
        self.config_dir.as_ref()
    }

    /// Get a reference to the cache directory
    #[must_use]
    pub const fn cache_dir(&self) -> Option<&PathBuf> {
        self.cache_dir.as_ref()
    }
}

impl Default for FrameworkTemplates {
    fn default() -> Self {
        Self::new().expect("Failed to create framework templates")
    }
}

impl Clone for FrameworkTemplates {
    fn clone(&self) -> Self {
        Self {
            env: Arc::clone(&self.env),
            config_dir: self.config_dir.clone(),
            cache_dir: self.cache_dir.clone(),
        }
    }
}

// Embedded fallback templates (always available)
// These are the default templates that ship with the framework
static EMBEDDED_TEMPLATES: phf::Map<&'static str, &'static str> = phf::phf_map! {
    // Forms
    "forms/form.html" => include_str!("defaults/forms/form.html"),
    "forms/field-wrapper.html" => include_str!("defaults/forms/field-wrapper.html"),
    "forms/input.html" => include_str!("defaults/forms/input.html"),
    "forms/textarea.html" => include_str!("defaults/forms/textarea.html"),
    "forms/select.html" => include_str!("defaults/forms/select.html"),
    "forms/checkbox.html" => include_str!("defaults/forms/checkbox.html"),
    "forms/radio-group.html" => include_str!("defaults/forms/radio-group.html"),
    "forms/submit-button.html" => include_str!("defaults/forms/submit-button.html"),
    "forms/help-text.html" => include_str!("defaults/forms/help-text.html"),
    "forms/label.html" => include_str!("defaults/forms/label.html"),
    "forms/csrf-input.html" => include_str!("defaults/forms/csrf-input.html"),
    // Validation
    "validation/field-errors.html" => include_str!("defaults/validation/field-errors.html"),
    "validation/validation-summary.html" => include_str!("defaults/validation/validation-summary.html"),
    // Flash messages
    "flash/container.html" => include_str!("defaults/flash/container.html"),
    "flash/message.html" => include_str!("defaults/flash/message.html"),
    // HTMX
    "htmx/oob-wrapper.html" => include_str!("defaults/htmx/oob-wrapper.html"),
    // Error pages
    "errors/400.html" => include_str!("defaults/errors/400.html"),
    "errors/401.html" => include_str!("defaults/errors/401.html"),
    "errors/403.html" => include_str!("defaults/errors/403.html"),
    "errors/404.html" => include_str!("defaults/errors/404.html"),
    "errors/422.html" => include_str!("defaults/errors/422.html"),
    "errors/500.html" => include_str!("defaults/errors/500.html"),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir_resolution() {
        let dir = FrameworkTemplates::get_config_dir();
        assert!(dir.is_some());
        let path = dir.unwrap();
        assert!(path.to_string_lossy().contains("acton-htmx"));
        assert!(path.to_string_lossy().contains("templates"));
        assert!(path.to_string_lossy().contains("framework"));
    }

    #[test]
    fn test_cache_dir_resolution() {
        let dir = FrameworkTemplates::get_cache_dir();
        assert!(dir.is_some());
        let path = dir.unwrap();
        assert!(path.to_string_lossy().contains("acton-htmx"));
    }

    #[test]
    fn test_embedded_templates_exist() {
        // All templates should have embedded fallbacks
        for name in TEMPLATE_NAMES {
            assert!(
                EMBEDDED_TEMPLATES.contains_key(name),
                "Missing embedded template: {name}"
            );
        }
    }

    #[test]
    fn test_framework_templates_creation() {
        let templates = FrameworkTemplates::new();
        assert!(templates.is_ok(), "Failed to create FrameworkTemplates");
    }

    #[test]
    fn test_render_csrf_input() {
        let templates = FrameworkTemplates::new().unwrap();
        let result = templates.render(
            "forms/csrf-input.html",
            minijinja::context! {
                token => "test-token-123",
            },
        );
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("test-token-123"));
        assert!(html.contains("_csrf_token"));
    }

    #[test]
    fn test_is_customized_returns_false_for_embedded() {
        let templates = FrameworkTemplates::new().unwrap();
        // Should return false since we're using embedded templates
        assert!(!templates.is_customized("forms/input.html"));
    }

    #[test]
    fn test_clone() {
        let templates = FrameworkTemplates::new().unwrap();
        let cloned = templates.clone();
        // Both should work
        assert!(templates.render("forms/csrf-input.html", minijinja::context! { token => "a" }).is_ok());
        assert!(cloned.render("forms/csrf-input.html", minijinja::context! { token => "b" }).is_ok());
    }
}

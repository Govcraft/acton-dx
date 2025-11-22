//! Template download and caching manager
//!
//! This module manages downloading scaffold templates from GitHub and caching them
//! in the XDG config directory for offline use and user customization.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// GitHub raw content base URL for templates
const GITHUB_RAW_BASE: &str = "https://raw.githubusercontent.com/Govcraft/acton-htmx/main/templates/scaffold";

/// Template files that should be downloaded
const TEMPLATE_FILES: &[&str] = &[
    "model.rs.hbs",
    "migration.sql.hbs",
    "form.rs.hbs",
    "handler.rs.hbs",
    "test.rs.hbs",
    "list.html.hbs",
    "show.html.hbs",
    "form.html.hbs",
    "_row.html.hbs",
    "_rows.html.hbs",
];

/// Template manager for downloading and caching scaffold templates
pub struct TemplateManager {
    cache_dir: PathBuf,
}

impl TemplateManager {
    /// Create a new template manager
    ///
    /// Templates are cached in `$XDG_CONFIG_HOME/acton-htmx/templates/scaffold/`
    /// or `~/.config/acton-htmx/templates/scaffold/` if `XDG_CONFIG_HOME` is not set.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache directory cannot be determined or created.
    pub fn new() -> Result<Self> {
        let cache_dir = Self::get_cache_dir()?;
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create template cache directory: {}", cache_dir.display()))?;

        Ok(Self { cache_dir })
    }

    /// Get the XDG config directory for templates
    fn get_cache_dir() -> Result<PathBuf> {
        let config_dir = if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config)
        } else {
            let home = std::env::var("HOME")
                .context("HOME environment variable not set")?;
            PathBuf::from(home).join(".config")
        };

        Ok(config_dir.join("acton-htmx").join("templates").join("scaffold"))
    }

    /// Ensure all templates are available locally
    ///
    /// This checks if templates exist in the cache directory. If any are missing,
    /// it downloads them from GitHub.
    ///
    /// # Errors
    ///
    /// Returns an error if templates cannot be downloaded or written to disk.
    pub fn ensure_templates(&self) -> Result<()> {
        let missing_templates = self.get_missing_templates();

        if missing_templates.is_empty() {
            return Ok(());
        }

        println!("Downloading {} scaffold templates from GitHub...", missing_templates.len());

        for template_name in missing_templates {
            self.download_template(&template_name)
                .with_context(|| format!("Failed to download template: {template_name}"))?;
        }

        println!("✓ Templates downloaded to {}", self.cache_dir.display());
        Ok(())
    }

    /// Get list of templates that are not yet cached locally
    fn get_missing_templates(&self) -> Vec<String> {
        TEMPLATE_FILES
            .iter()
            .filter(|&&name| !self.cache_dir.join(name).exists())
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
        let content = body.read_to_vec()
            .context("Failed to read template content")?;

        let dest_path = self.cache_dir.join(name);
        fs::write(&dest_path, content)
            .with_context(|| format!("Failed to write template to {}", dest_path.display()))?;

        println!("  ✓ {name}");
        Ok(())
    }

    /// Get the path to a cached template file
    ///
    /// # Errors
    ///
    /// Returns an error if the template doesn't exist in the cache.
    pub fn get_template_path(&self, name: &str) -> Result<PathBuf> {
        let path = self.cache_dir.join(name);

        if !path.exists() {
            anyhow::bail!("Template not found in cache: {name}. Run with --update to download templates.");
        }

        Ok(path)
    }

    /// Get the cache directory path
    #[must_use]
    pub const fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Force re-download all templates from GitHub
    ///
    /// This is useful for getting the latest template updates.
    ///
    /// # Errors
    ///
    /// Returns an error if templates cannot be downloaded.
    pub fn update_templates(&self) -> Result<()> {
        println!("Updating scaffold templates from GitHub...");

        for &template_name in TEMPLATE_FILES {
            self.download_template(template_name)?;
        }

        println!("✓ All templates updated");
        Ok(())
    }
}

impl Default for TemplateManager {
    fn default() -> Self {
        Self::new().expect("Failed to create template manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_dir_uses_xdg() {
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/test-xdg");
        let dir = TemplateManager::get_cache_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/test-xdg/acton-htmx/templates/scaffold"));
    }

    #[test]
    fn test_cache_dir_falls_back_to_home() {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", "/tmp/test-home");
        let dir = TemplateManager::get_cache_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/test-home/.config/acton-htmx/templates/scaffold"));
    }

    #[test]
    fn test_template_files_list() {
        assert_eq!(TEMPLATE_FILES.len(), 10);
        assert!(TEMPLATE_FILES.contains(&"model.rs.hbs"));
        assert!(TEMPLATE_FILES.contains(&"handler.rs.hbs"));
    }
}

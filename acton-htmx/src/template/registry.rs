//! Template registry with caching support
//!
//! Provides a registry for managing compiled templates with optional caching
//! for improved performance.

#![allow(dead_code)]

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Template registry for caching compiled templates
///
/// In development mode, templates are recompiled on every request.
/// In production mode, templates are cached after first compilation.
#[derive(Clone)]
pub struct TemplateRegistry {
    cache: Arc<RwLock<HashMap<String, String>>>,
    cache_enabled: bool,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    /// Create a new template registry
    ///
    /// Caching is disabled in debug builds and enabled in release builds.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_enabled: !cfg!(debug_assertions),
        }
    }

    /// Create a new template registry with explicit cache control
    #[must_use]
    pub fn with_caching(cache_enabled: bool) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_enabled,
        }
    }

    /// Get a cached template
    ///
    /// Returns `None` if caching is disabled or template not found.
    pub fn get(&self, name: &str) -> Option<String> {
        if !self.cache_enabled {
            return None;
        }

        self.cache.read().get(name).cloned()
    }

    /// Cache a compiled template
    ///
    /// No-op if caching is disabled.
    pub fn insert(&self, name: String, content: String) {
        if !self.cache_enabled {
            return;
        }

        self.cache.write().insert(name, content);
    }

    /// Clear all cached templates
    ///
    /// Useful for development hot-reload or cache invalidation.
    pub fn clear(&self) {
        self.cache.write().clear();
    }

    /// Check if caching is enabled
    #[must_use]
    pub const fn is_caching_enabled(&self) -> bool {
        self.cache_enabled
    }

    /// Get the number of cached templates
    #[must_use]
    pub fn cache_size(&self) -> usize {
        self.cache.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = TemplateRegistry::new();
        assert!(registry.cache_size() == 0);
    }

    #[test]
    fn test_registry_with_caching_enabled() {
        let registry = TemplateRegistry::with_caching(true);
        registry.insert("test".to_string(), "<html></html>".to_string());
        assert_eq!(registry.cache_size(), 1);
        assert!(registry.get("test").is_some());
    }

    #[test]
    fn test_registry_with_caching_disabled() {
        let registry = TemplateRegistry::with_caching(false);
        registry.insert("test".to_string(), "<html></html>".to_string());
        assert_eq!(registry.cache_size(), 0);
        assert!(registry.get("test").is_none());
    }

    #[test]
    fn test_registry_clear() {
        let registry = TemplateRegistry::with_caching(true);
        registry.insert("test1".to_string(), "<html></html>".to_string());
        registry.insert("test2".to_string(), "<html></html>".to_string());
        assert_eq!(registry.cache_size(), 2);

        registry.clear();
        assert_eq!(registry.cache_size(), 0);
    }
}

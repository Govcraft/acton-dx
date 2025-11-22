//! acton-htmx CLI library

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::multiple_crate_versions)]

pub mod scaffold;
pub mod template_manager;
pub mod templates;

pub use scaffold::{FieldDefinition, FieldType, ScaffoldGenerator, TemplateHelpers};
pub use template_manager::TemplateManager;
pub use templates::ProjectTemplate;

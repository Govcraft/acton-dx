//! CRUD scaffold generator orchestrator
//!
//! This module coordinates the generation of all files for a CRUD resource.
//! It uses the field type system, template system, and helpers to generate:
//! - Models
//! - Migrations
//! - Forms
//! - Handlers
//! - Templates
//! - Tests
//! - Route registration

use super::field_type::FieldDefinition;
use super::helpers::TemplateHelpers;
use super::templates::TemplateRegistry;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// CRUD scaffold generator
#[allow(dead_code)] // Fields will be used in Week 2-3 implementation
pub struct ScaffoldGenerator {
    /// Model name (e.g., "Post", "`UserProfile`")
    model_name: String,
    /// Field definitions
    fields: Vec<FieldDefinition>,
    /// Template registry
    templates: TemplateRegistry,
    /// Project root directory
    project_root: PathBuf,
}

impl ScaffoldGenerator {
    /// Create a new scaffold generator
    ///
    /// # Arguments
    ///
    /// * `model_name` - Name of the model (e.g., "Post", "`UserProfile`")
    /// * `field_specs` - Field specifications (e.g., `["title:string", "content:text"]`)
    /// * `project_root` - Project root directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Model name is not `PascalCase` (doesn't start with uppercase)
    /// - Field specifications cannot be parsed
    /// - No fields are specified
    /// - Template registry initialization fails
    pub fn new(
        model_name: String,
        field_specs: &[String],
        project_root: PathBuf,
    ) -> Result<Self> {
        // Validate model name
        if !model_name.chars().next().unwrap_or('0').is_uppercase() {
            anyhow::bail!("Model name must be PascalCase (start with uppercase): '{model_name}'");
        }

        // Parse field definitions
        let fields = field_specs
            .iter()
            .map(|spec| FieldDefinition::parse(spec))
            .collect::<Result<Vec<_>>>()
            .context("Failed to parse field definitions")?;

        if fields.is_empty() {
            anyhow::bail!("At least one field must be specified");
        }

        let templates = TemplateRegistry::new()?;

        Ok(Self {
            model_name,
            fields,
            templates,
            project_root,
        })
    }

    /// Generate all CRUD files
    ///
    /// This orchestrates the generation of:
    /// 1. Model file (src/models/{model}.rs)
    /// 2. Migration file (migrations/{timestamp}_create_{table}.sql)
    /// 3. Form file (src/forms/{model}.rs)
    /// 4. Handler file (src/handlers/{model}s.rs)
    /// 5. Template files (templates/{model}s/*.html)
    /// 6. Test file (`tests/{model}s_test.rs`)
    ///
    /// # Errors
    ///
    /// Returns an error if template rendering fails for any file
    pub fn generate(&self) -> Result<Vec<GeneratedFile>> {
        let mut generated_files = vec![
            self.generate_model()?,
            self.generate_migration()?,
            self.generate_forms()?,
            self.generate_handlers()?,
            self.generate_tests()?,
        ];

        // Add all template files
        generated_files.extend(self.generate_templates()?);

        Ok(generated_files)
    }

    /// Get model metadata for templates
    ///
    /// This generates all the template variables needed for code generation
    fn model_metadata(&self) -> serde_json::Value {
        use super::field_type::FieldType;

        let table_name = TemplateHelpers::to_table_name(&self.model_name);
        let model_snake = TemplateHelpers::to_snake_case(&self.model_name);

        let enums = self.collect_enums();
        let (relations, foreign_keys) = self.collect_relations();
        let (unique_fields, indexed_fields) = self.collect_indexes();
        let fields = self.build_field_metadata();

        // Check for special field types
        let has_date_fields = self.fields.iter().any(|f| {
            matches!(
                f.field_type,
                FieldType::Date | FieldType::DateTime | FieldType::Timestamp
            )
        });
        let has_decimal = self.fields.iter().any(|f| matches!(f.field_type, FieldType::Decimal));
        let has_uuid = self.fields.iter().any(|f| matches!(f.field_type, FieldType::Uuid));
        let has_enum = !enums.is_empty();

        serde_json::json!({
            "model_name": self.model_name,
            "model_snake": model_snake,
            "model_plural": TemplateHelpers::pluralize(&self.model_name),
            "table_name": table_name,
            "route_path": TemplateHelpers::to_route_path(&self.model_name),
            "title": TemplateHelpers::to_title(&self.model_name),
            "plural_title": TemplateHelpers::to_plural_title(&self.model_name),
            "fields": fields,
            "relations": relations,
            "foreign_keys": foreign_keys,
            "unique_fields": unique_fields,
            "indexed_fields": indexed_fields,
            "enums": enums,
            "has_date_fields": has_date_fields,
            "has_decimal": has_decimal,
            "has_uuid": has_uuid,
            "has_enum": has_enum,
        })
    }

    /// Collect enum type definitions from fields
    fn collect_enums(&self) -> Vec<serde_json::Value> {
        use super::field_type::FieldType;

        self.fields
            .iter()
            .filter_map(|field| {
                if let FieldType::Enum { name, variants } = &field.field_type {
                    Some(serde_json::json!({
                        "name": name,
                        "variants": variants,
                    }))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collect relations and foreign key definitions
    fn collect_relations(&self) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
        use super::field_type::FieldType;

        let mut relations = Vec::new();
        let mut foreign_keys = Vec::new();

        for field in &self.fields {
            if let FieldType::Reference { model } = &field.field_type {
                let referenced_table = TemplateHelpers::to_table_name(model);
                let relation_name = TemplateHelpers::to_pascal_case(&field.name);
                let field_column = TemplateHelpers::to_foreign_key(&field.name, model);

                relations.push(serde_json::json!({
                    "field_name": field.name,
                    "relation_name": relation_name,
                    "referenced_table": referenced_table,
                    "field_column": field_column,
                }));

                foreign_keys.push(serde_json::json!({
                    "field_name": field.name,
                    "column_name": field_column,
                    "referenced_table": referenced_table,
                }));
            }
        }

        (relations, foreign_keys)
    }

    /// Collect unique and indexed field definitions
    fn collect_indexes(&self) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
        let unique_fields = self
            .fields
            .iter()
            .filter(|f| f.unique)
            .map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "column_name": TemplateHelpers::to_snake_case(&f.name),
                })
            })
            .collect();

        let indexed_fields = self
            .fields
            .iter()
            .filter(|f| f.indexed && !f.unique)
            .map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "column_name": TemplateHelpers::to_snake_case(&f.name),
                })
            })
            .collect();

        (unique_fields, indexed_fields)
    }

    /// Build field metadata with validation and default values
    fn build_field_metadata(&self) -> Vec<serde_json::Value> {
        use super::field_type::FieldType;

        self.fields
            .iter()
            .map(|f| {
                let column_name = if let FieldType::Reference { model } = &f.field_type {
                    TemplateHelpers::to_foreign_key(&f.name, model)
                } else {
                    TemplateHelpers::to_snake_case(&f.name)
                };

                let validations = Self::get_validations(f);
                let default_value = Self::get_default_value(f);

                serde_json::json!({
                    "name": f.name,
                    "column_name": column_name,
                    "rust_type": f.rust_type(),
                    "sql_type": f.sql_type(),
                    "optional": f.optional,
                    "unique": f.unique,
                    "indexed": f.indexed,
                    "validations": validations,
                    "default_value": default_value,
                })
            })
            .collect()
    }

    /// Get validation rules for a field
    fn get_validations(field: &FieldDefinition) -> Vec<String> {
        use super::field_type::FieldType;

        let mut validations = Vec::new();

        match &field.field_type {
            FieldType::String if !field.optional => {
                validations.push("length(min = 1, max = 255)".to_string());
            }
            FieldType::Text if !field.optional => {
                validations.push("length(min = 1)".to_string());
            }
            _ => {}
        }

        // Add email validation if field name suggests email
        if field.name.to_lowercase().contains("email") {
            validations.push("email".to_string());
        }

        validations
    }

    /// Get default value for testing
    fn get_default_value(field: &FieldDefinition) -> String {
        use super::field_type::FieldType;

        match &field.field_type {
            FieldType::String | FieldType::Text => "\"test\".to_string()".to_string(),
            FieldType::Integer | FieldType::BigInt => "0".to_string(),
            FieldType::Boolean => "false".to_string(),
            FieldType::Float | FieldType::Double => "0.0".to_string(),
            FieldType::Decimal => "rust_decimal::Decimal::ZERO".to_string(),
            FieldType::Date => "chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()".to_string(),
            FieldType::DateTime => {
                "chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap()".to_string()
            }
            FieldType::Timestamp => "chrono::Utc::now()".to_string(),
            FieldType::Json => "serde_json::json!({})".to_string(),
            FieldType::Uuid => "uuid::Uuid::new_v4()".to_string(),
            FieldType::Reference { .. } => "1".to_string(),
            FieldType::Array { .. } => "vec![]".to_string(),
            FieldType::Enum { name, variants } => {
                let variant = variants.first().map_or("Unknown", String::as_str);
                format!("{name}::{variant}")
            }
        }
    }

    /// Generate `SeaORM` model file
    fn generate_model(&self) -> Result<GeneratedFile> {
        let metadata = self.model_metadata();
        let content = self.templates.render("model", &metadata)?;

        let model_snake = TemplateHelpers::to_snake_case(&self.model_name);
        let path = PathBuf::from(format!("src/models/{model_snake}.rs"));

        let model_name = &self.model_name;
        Ok(GeneratedFile {
            path,
            content,
            description: format!("SeaORM model for {model_name}"),
        })
    }

    /// Generate database migration file
    fn generate_migration(&self) -> Result<GeneratedFile> {
        let metadata = self.model_metadata();
        let content = self.templates.render("migration", &metadata)?;

        let table_name = TemplateHelpers::to_table_name(&self.model_name);
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let path = PathBuf::from(format!("migrations/{timestamp}_{table_name}.sql"));

        Ok(GeneratedFile {
            path,
            content,
            description: format!("Database migration for {table_name} table"),
        })
    }

    /// Generate form struct file
    fn generate_forms(&self) -> Result<GeneratedFile> {
        let metadata = self.model_metadata();
        let content = self.templates.render("form", &metadata)?;

        let model_snake = TemplateHelpers::to_snake_case(&self.model_name);
        let path = PathBuf::from(format!("src/forms/{model_snake}.rs"));

        let model_name = &self.model_name;
        Ok(GeneratedFile {
            path,
            content,
            description: format!("Form validation for {model_name}"),
        })
    }

    /// Generate handler file with all CRUD operations
    fn generate_handlers(&self) -> Result<GeneratedFile> {
        let metadata = self.model_metadata();
        let content = self.templates.render("handler", &metadata)?;

        let model_snake = TemplateHelpers::to_snake_case(&self.model_name);
        let plural = TemplateHelpers::pluralize(&model_snake);
        let path = PathBuf::from(format!("src/handlers/{plural}.rs"));

        let model_name = &self.model_name;
        Ok(GeneratedFile {
            path,
            content,
            description: format!("HTMX handlers for {model_name}"),
        })
    }

    /// Generate integration tests
    fn generate_tests(&self) -> Result<GeneratedFile> {
        let metadata = self.model_metadata();
        let content = self.templates.render("test", &metadata)?;

        let model_snake = TemplateHelpers::to_snake_case(&self.model_name);
        let plural = TemplateHelpers::pluralize(&model_snake);
        let path = PathBuf::from(format!("tests/{plural}_test.rs"));

        let model_name = &self.model_name;
        Ok(GeneratedFile {
            path,
            content,
            description: format!("Integration tests for {model_name}"),
        })
    }

    /// Generate all Askama templates
    fn generate_templates(&self) -> Result<Vec<GeneratedFile>> {
        use crate::template_manager::TemplateManager;

        let metadata = self.model_metadata();
        let model_snake = TemplateHelpers::to_snake_case(&self.model_name);
        let plural = TemplateHelpers::pluralize(&model_snake);

        // Get template manager for loading template files
        let template_manager = TemplateManager::new()?;

        // Create a temporary Handlebars instance for template rendering
        let mut hb = handlebars::Handlebars::new();
        hb.register_escape_fn(handlebars::no_escape);

        let mut templates = Vec::new();

        // Helper to load and render a template file
        let render_template_file = |filename: &str, description: &str, dest: &str| -> Result<GeneratedFile> {
            let template_path = template_manager.get_template_path(filename)?;
            let template_content = std::fs::read_to_string(&template_path)?;
            let rendered = hb.render_template(&template_content, &metadata)?;

            Ok(GeneratedFile {
                path: PathBuf::from(dest),
                content: rendered,
                description: description.to_string(),
            })
        };

        // List template
        templates.push(render_template_file(
            "list.html.hbs",
            &format!("List view for {}", self.model_name),
            &format!("templates/{plural}/list.html"),
        )?);

        // Show template
        templates.push(render_template_file(
            "show.html.hbs",
            &format!("Show view for {}", self.model_name),
            &format!("templates/{plural}/show.html"),
        )?);

        // Form template (new and edit)
        templates.push(render_template_file(
            "form.html.hbs",
            &format!("Form view for {}", self.model_name),
            &format!("templates/{plural}/form.html"),
        )?);

        // Row partial (single row)
        templates.push(render_template_file(
            "_row.html.hbs",
            &format!("Row partial for {}", self.model_name),
            &format!("templates/{plural}/_row.html"),
        )?);

        // Rows partial (multiple rows)
        templates.push(render_template_file(
            "_rows.html.hbs",
            &format!("Rows partial for {}", self.model_name),
            &format!("templates/{plural}/_rows.html"),
        )?);

        Ok(templates)
    }
}

/// Represents a generated file
#[derive(Debug)]
pub struct GeneratedFile {
    /// Relative path from project root
    pub path: PathBuf,
    /// File content
    pub content: String,
    /// File description for user feedback
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_generator() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string(), "content:text".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        );

        assert!(generator.is_ok());
        let generator = generator.unwrap();
        assert_eq!(generator.model_name, "Post");
        assert_eq!(generator.fields.len(), 2);
    }

    #[test]
    fn test_invalid_model_name() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string()];
        let result = ScaffoldGenerator::new(
            "post".to_string(), // lowercase - should fail
            &fields,
            temp_dir.path().to_path_buf(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_no_fields() {
        let temp_dir = tempdir().unwrap();
        let fields: Vec<String> = vec![]; // no fields - should fail
        let result = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_model_metadata() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let metadata = generator.model_metadata();
        assert_eq!(metadata["model_name"], "Post");
        assert_eq!(metadata["table_name"], "posts");
        assert_eq!(metadata["route_path"], "/posts");
    }

    #[test]
    fn test_generate_model() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string(), "content:text".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_model().unwrap();
        assert!(generated.path.to_string_lossy().contains("post.rs"));
        assert!(generated.content.contains("pub struct Model"));
        assert!(generated.content.contains("pub title: String"));
        assert!(generated.content.contains("pub content: String"));
    }

    #[test]
    fn test_generate_migration() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string(), "published:boolean".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_migration().unwrap();
        assert!(generated.path.to_string_lossy().contains("posts.sql"));
        assert!(generated.content.contains("CREATE TABLE posts"));
        assert!(generated.content.contains("title VARCHAR(255) NOT NULL"));
        assert!(generated.content.contains("published BOOLEAN NOT NULL"));
    }

    #[test]
    fn test_generate_forms() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string(), "age:integer:optional".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_forms().unwrap();
        assert!(generated.path.to_string_lossy().contains("post.rs"));
        assert!(generated.content.contains("pub struct PostForm"));
        assert!(generated.content.contains("pub title: String"));
        assert!(generated.content.contains("pub age: Option<i32>"));
    }

    #[test]
    fn test_generate_with_enum() {
        let temp_dir = tempdir().unwrap();
        let fields = vec![
            "title:string".to_string(),
            "status:enum:Draft,Published,Archived".to_string(),
        ];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_model().unwrap();
        assert!(generated.content.contains("pub enum Status"));
        assert!(generated.content.contains("Draft"));
        assert!(generated.content.contains("Published"));
        assert!(generated.content.contains("Archived"));
    }

    #[test]
    fn test_generate_with_references() {
        let temp_dir = tempdir().unwrap();
        let fields = vec![
            "content:text".to_string(),
            "author:references:User".to_string(),
            "post:references:Post".to_string(),
        ];
        let generator = ScaffoldGenerator::new(
            "Comment".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_model().unwrap();
        assert!(generated.content.contains("pub author: UserId"));
        assert!(generated.content.contains("pub post: PostId"));

        let migration = generator.generate_migration().unwrap();
        assert!(migration.content.contains("FOREIGN KEY (author_id)"));
        assert!(migration.content.contains("REFERENCES users(id)"));
    }

    #[test]
    fn test_generate_with_unique_and_indexed() {
        let temp_dir = tempdir().unwrap();
        let fields = vec![
            "email:string:unique".to_string(),
            "username:string:indexed".to_string(),
        ];
        let generator = ScaffoldGenerator::new(
            "User".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_model().unwrap();
        assert!(generated.content.contains("#[sea_orm(unique)]"));
        assert!(generated.content.contains("#[sea_orm(indexed)]"));

        let migration = generator.generate_migration().unwrap();
        assert!(migration.content.contains("users_email_unique"));
        assert!(migration.content.contains("users_username_idx"));
    }

    #[test]
    fn test_complete_generation() {
        let temp_dir = tempdir().unwrap();
        let fields = vec![
            "title:string:unique".to_string(),
            "content:text".to_string(),
            "published:boolean".to_string(),
            "author:references:User".to_string(),
        ];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let files = generator.generate().unwrap();
        assert_eq!(files.len(), 10); // model, migration, form, handler, test, + 5 templates

        // Verify key files
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("models/post.rs")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("migrations/") && f.path.to_string_lossy().contains("posts.sql")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("forms/post.rs")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("handlers/posts.rs")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("tests/posts_test.rs")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("templates/posts/list.html")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("templates/posts/show.html")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("templates/posts/form.html")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("templates/posts/_row.html")));
        assert!(files.iter().any(|f| f.path.to_string_lossy().contains("templates/posts/_rows.html")));
    }

    #[test]
    fn test_handler_generation() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string(), "content:text".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_handlers().unwrap();
        assert!(generated.path.to_string_lossy().contains("handlers/posts.rs"));
        assert!(generated.content.contains("pub async fn list("));
        assert!(generated.content.contains("pub async fn show("));
        assert!(generated.content.contains("pub async fn new("));
        assert!(generated.content.contains("pub async fn create("));
        assert!(generated.content.contains("pub async fn edit("));
        assert!(generated.content.contains("pub async fn update("));
        assert!(generated.content.contains("pub async fn delete("));
        assert!(generated.content.contains("pub async fn search("));
    }

    #[test]
    fn test_template_generation() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let templates = generator.generate_templates().unwrap();
        assert_eq!(templates.len(), 5);

        let list_template = templates.iter().find(|t| t.path.to_string_lossy().contains("list.html")).unwrap();
        assert!(list_template.content.contains("{% extends \"base.html\" %}"));
        assert!(list_template.content.contains("hx-get"));
        assert!(list_template.content.contains("hx-target"));
    }

    #[test]
    fn test_test_generation() {
        let temp_dir = tempdir().unwrap();
        let fields = vec!["title:string".to_string()];
        let generator = ScaffoldGenerator::new(
            "Post".to_string(),
            &fields,
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        let generated = generator.generate_tests().unwrap();
        assert!(generated.path.to_string_lossy().contains("tests/posts_test.rs"));
        assert!(generated.content.contains("test_list_posts"));
        assert!(generated.content.contains("test_create_post"));
        assert!(generated.content.contains("test_show_post"));
        assert!(generated.content.contains("test_update_post"));
        assert!(generated.content.contains("test_delete_post"));
        assert!(generated.content.contains("test_validation_errors"));
    }
}

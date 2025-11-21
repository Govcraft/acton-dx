//! Field type definitions and parser for CRUD scaffolding
//!
//! This module provides the type system for field definitions in the scaffold generator.
//! It supports all common database types, relationships, and modifiers.
//!
//! # Supported Field Types
//!
//! ## Primitive Types
//! - `string` - VARCHAR(255), Rust `String`
//! - `text` - TEXT, Rust `String`
//! - `integer` - INTEGER, Rust `i32`
//! - `bigint` - BIGINT, Rust `i64`
//! - `boolean` - BOOLEAN, Rust `bool`
//! - `float` - FLOAT, Rust `f32`
//! - `double` - DOUBLE, Rust `f64`
//! - `decimal` - DECIMAL, Rust `rust_decimal::Decimal`
//! - `date` - DATE, Rust `chrono::NaiveDate`
//! - `datetime` - TIMESTAMP, Rust `chrono::NaiveDateTime`
//! - `timestamp` - TIMESTAMP WITH TIMEZONE, Rust `chrono::DateTime<Utc>`
//! - `json` - JSON, Rust `serde_json::Value`
//! - `uuid` - UUID, Rust `uuid::Uuid`
//!
//! ## Relationships
//! - `references:Model` - Foreign key to another model
//! - `belongs_to:Model` - Alias for references
//!
//! ## Collections
//! - `array:type` - Array of primitive type
//!
//! ## Enums
//! - `enum:Value1,Value2,Value3` - Enumeration type
//!
//! ## Modifiers
//! - `:optional` - Makes field nullable (Option<T>)
//! - `:unique` - Adds unique constraint
//! - `:indexed` - Adds database index
//!
//! # Examples
//!
//! ```text
//! title:string              → String
//! content:text              → String (TEXT column)
//! age:integer:optional      → Option<i32>
//! email:string:unique       → String (with unique constraint)
//! author:references:User    → Foreign key to users table
//! published:boolean         → bool
//! published_at:datetime:optional → Option<NaiveDateTime>
//! tags:array:string         → Vec<String>
//! status:enum:Draft,Published,Archived → Status enum
//! ```

use anyhow::{anyhow, Result};
use std::fmt;

/// Represents a field definition parsed from user input
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDefinition {
    /// Field name (e.g., "title", "`published_at`")
    pub name: String,
    /// Field type
    pub field_type: FieldType,
    /// Whether field is nullable
    pub optional: bool,
    /// Whether field has unique constraint
    pub unique: bool,
    /// Whether field is indexed
    pub indexed: bool,
}

/// Field type enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    /// String with max length (VARCHAR)
    String,
    /// Text (unlimited length)
    Text,
    /// 32-bit integer
    Integer,
    /// 64-bit integer
    BigInt,
    /// Boolean
    Boolean,
    /// 32-bit float
    Float,
    /// 64-bit float
    Double,
    /// Decimal number
    Decimal,
    /// Date (no time)
    Date,
    /// `DateTime` (no timezone)
    DateTime,
    /// Timestamp (with timezone)
    Timestamp,
    /// JSON value
    Json,
    /// UUID
    Uuid,
    /// Foreign key reference to another model
    Reference {
        /// Referenced model name (e.g., "User")
        model: String,
    },
    /// Array of values
    Array {
        /// Element type
        element_type: Box<FieldType>,
    },
    /// Enum type
    Enum {
        /// Enum name (derived from field name)
        name: String,
        /// Enum variants
        variants: Vec<String>,
    },
}

impl FieldDefinition {
    /// Parse a field definition from a string
    ///
    /// Format: `name:type[:modifier]*`
    ///
    /// # Examples
    ///
    /// ```
    /// # use acton_htmx::scaffold::field_type::FieldDefinition;
    /// let field = FieldDefinition::parse("title:string").unwrap();
    /// assert_eq!(field.name, "title");
    ///
    /// let field = FieldDefinition::parse("age:integer:optional").unwrap();
    /// assert!(field.optional);
    ///
    /// let field = FieldDefinition::parse("email:string:unique:indexed").unwrap();
    /// assert!(field.unique);
    /// assert!(field.indexed);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Field definition has fewer than 2 parts (missing name or type)
    /// - Field name is empty or not a valid Rust identifier
    /// - Field type is unknown or malformed
    /// - Enum type has no variants or invalid variant names
    /// - Unknown modifier is specified
    pub fn parse(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.split(':').collect();

        if parts.len() < 2 {
            return Err(anyhow!(
                "Invalid field definition: '{input}'. Expected format: name:type[:modifiers]"
            ));
        }

        let name = parts[0].trim().to_string();
        if name.is_empty() {
            return Err(anyhow!("Field name cannot be empty"));
        }

        // Validate field name (must be valid Rust identifier)
        if !name.chars().next().unwrap_or('0').is_alphabetic()
            || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            return Err(anyhow!(
                "Invalid field name: '{name}'. Must be a valid Rust identifier (alphanumeric + underscore)"
            ));
        }

        // Reconstruct type string (might contain colons for references, arrays, enums)
        // Find where modifiers start (optional, unique, indexed)
        let modifier_keywords = ["optional", "unique", "indexed", "index"];
        let mut type_end_idx = parts.len();
        for (idx, part) in parts.iter().enumerate().skip(1) {
            if modifier_keywords.contains(&part.trim().to_lowercase().as_str()) {
                type_end_idx = idx;
                break;
            }
        }

        // Reconstruct type string from parts[1..type_end_idx]
        let type_str = parts[1..type_end_idx].join(":");
        let field_type = Self::parse_type(&type_str, &name)?;

        let mut optional = false;
        let mut unique = false;
        let mut indexed = false;

        // Parse modifiers
        for modifier in parts.iter().skip(type_end_idx) {
            match modifier.trim().to_lowercase().as_str() {
                "optional" => optional = true,
                "unique" => unique = true,
                "indexed" | "index" => indexed = true,
                unknown => {
                    return Err(anyhow!(
                        "Unknown modifier: '{unknown}'. Valid modifiers: optional, unique, indexed"
                    ));
                }
            }
        }

        Ok(Self {
            name,
            field_type,
            optional,
            unique,
            indexed,
        })
    }

    /// Parse field type from string
    fn parse_type(type_str: &str, field_name: &str) -> Result<FieldType> {
        // Check for array type: array:element_type
        if let Some(element_type_str) = type_str.strip_prefix("array:") {
            let element_type = Self::parse_type(element_type_str, field_name)?;
            return Ok(FieldType::Array {
                element_type: Box::new(element_type),
            });
        }

        // Check for enum type: enum:Variant1,Variant2,Variant3
        if let Some(variants_str) = type_str.strip_prefix("enum:") {
            let variants: Vec<String> = variants_str
                .split(',')
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect();

            if variants.is_empty() {
                return Err(anyhow!(
                    "Enum type must have at least one variant. Format: enum:Variant1,Variant2"
                ));
            }

            // Validate variant names
            for variant in &variants {
                if !variant.chars().next().unwrap_or('0').is_uppercase()
                    || !variant.chars().all(char::is_alphanumeric)
                {
                    return Err(anyhow!(
                        "Invalid enum variant: '{variant}'. Must be PascalCase alphanumeric"
                    ));
                }
            }

            // Generate enum name from field name (convert to PascalCase)
            let enum_name = super::helpers::TemplateHelpers::to_pascal_case(field_name);

            return Ok(FieldType::Enum {
                name: enum_name,
                variants,
            });
        }

        // Check for reference type: references:Model or belongs_to:Model
        if let Some(model) = type_str.strip_prefix("references:") {
            return Ok(FieldType::Reference {
                model: model.trim().to_string(),
            });
        }
        if let Some(model) = type_str.strip_prefix("belongs_to:") {
            return Ok(FieldType::Reference {
                model: model.trim().to_string(),
            });
        }

        // Parse primitive types
        match type_str.to_lowercase().as_str() {
            "string" => Ok(FieldType::String),
            "text" => Ok(FieldType::Text),
            "integer" | "int" | "i32" => Ok(FieldType::Integer),
            "bigint" | "biginteger" | "i64" => Ok(FieldType::BigInt),
            "boolean" | "bool" => Ok(FieldType::Boolean),
            "float" | "f32" => Ok(FieldType::Float),
            "double" | "f64" => Ok(FieldType::Double),
            "decimal" => Ok(FieldType::Decimal),
            "date" => Ok(FieldType::Date),
            "datetime" => Ok(FieldType::DateTime),
            "timestamp" => Ok(FieldType::Timestamp),
            "json" | "jsonb" => Ok(FieldType::Json),
            "uuid" => Ok(FieldType::Uuid),
            unknown => Err(anyhow!(
                "Unknown field type: '{unknown}'. Supported types: string, text, integer, bigint, boolean, float, double, decimal, date, datetime, timestamp, json, uuid, references:Model, array:type, enum:Variant1,Variant2"
            )),
        }
    }

    /// Get Rust type string for this field
    ///
    /// # Examples
    ///
    /// ```
    /// # use acton_htmx::scaffold::field_type::FieldDefinition;
    /// let field = FieldDefinition::parse("title:string").unwrap();
    /// assert_eq!(field.rust_type(), "String");
    ///
    /// let field = FieldDefinition::parse("age:integer:optional").unwrap();
    /// assert_eq!(field.rust_type(), "Option<i32>");
    ///
    /// let field = FieldDefinition::parse("tags:array:string").unwrap();
    /// assert_eq!(field.rust_type(), "Vec<String>");
    /// ```
    #[must_use]
    pub fn rust_type(&self) -> String {
        let base_type = self.field_type.rust_type();
        if self.optional {
            format!("Option<{base_type}>")
        } else {
            base_type
        }
    }

    /// Get SQL type string for this field
    ///
    /// # Examples
    ///
    /// ```
    /// # use acton_htmx::scaffold::field_type::FieldDefinition;
    /// let field = FieldDefinition::parse("title:string").unwrap();
    /// assert_eq!(field.sql_type(), "VARCHAR(255)");
    ///
    /// let field = FieldDefinition::parse("content:text").unwrap();
    /// assert_eq!(field.sql_type(), "TEXT");
    /// ```
    #[must_use]
    pub fn sql_type(&self) -> String {
        self.field_type.sql_type()
    }
}

impl FieldType {
    /// Get Rust type string for this field type
    #[must_use]
    pub fn rust_type(&self) -> String {
        match self {
            Self::String | Self::Text => "String".to_string(),
            Self::Integer => "i32".to_string(),
            Self::BigInt => "i64".to_string(),
            Self::Boolean => "bool".to_string(),
            Self::Float => "f32".to_string(),
            Self::Double => "f64".to_string(),
            Self::Decimal => "rust_decimal::Decimal".to_string(),
            Self::Date => "chrono::NaiveDate".to_string(),
            Self::DateTime => "chrono::NaiveDateTime".to_string(),
            Self::Timestamp => "chrono::DateTime<chrono::Utc>".to_string(),
            Self::Json => "serde_json::Value".to_string(),
            Self::Uuid => "uuid::Uuid".to_string(),
            Self::Reference { model } => format!("{model}Id"),
            Self::Array { element_type } => {
                let inner = element_type.rust_type();
                format!("Vec<{inner}>")
            }
            Self::Enum { name, .. } => name.clone(),
        }
    }

    /// Get SQL type string for this field type
    #[must_use]
    pub fn sql_type(&self) -> String {
        match self {
            Self::String => "VARCHAR(255)".to_string(),
            Self::Text => "TEXT".to_string(),
            Self::Integer => "INTEGER".to_string(),
            Self::BigInt | Self::Reference { .. } => "BIGINT".to_string(),
            Self::Boolean => "BOOLEAN".to_string(),
            Self::Float => "REAL".to_string(),
            Self::Double => "DOUBLE PRECISION".to_string(),
            Self::Decimal => "DECIMAL(19,4)".to_string(),
            Self::Date => "DATE".to_string(),
            Self::DateTime => "TIMESTAMP".to_string(),
            Self::Timestamp => "TIMESTAMP WITH TIME ZONE".to_string(),
            Self::Json => "JSONB".to_string(),
            Self::Uuid => "UUID".to_string(),
            Self::Array { element_type } => {
                let inner = element_type.sql_type();
                format!("{inner}[]")
            }
            Self::Enum { .. } => "VARCHAR(50)".to_string(), // Store as string by default
        }
    }
}

impl fmt::Display for FieldDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = &self.name;
        let field_type = &self.field_type;
        write!(f, "{name}:{field_type}")?;
        if self.optional {
            write!(f, ":optional")?;
        }
        if self.unique {
            write!(f, ":unique")?;
        }
        if self.indexed {
            write!(f, ":indexed")?;
        }
        Ok(())
    }
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Text => write!(f, "text"),
            Self::Integer => write!(f, "integer"),
            Self::BigInt => write!(f, "bigint"),
            Self::Boolean => write!(f, "boolean"),
            Self::Float => write!(f, "float"),
            Self::Double => write!(f, "double"),
            Self::Decimal => write!(f, "decimal"),
            Self::Date => write!(f, "date"),
            Self::DateTime => write!(f, "datetime"),
            Self::Timestamp => write!(f, "timestamp"),
            Self::Json => write!(f, "json"),
            Self::Uuid => write!(f, "uuid"),
            Self::Reference { model } => write!(f, "references:{model}"),
            Self::Array { element_type } => write!(f, "array:{element_type}"),
            Self::Enum { name, variants } => {
                let joined = variants.join(",");
                write!(f, "enum:{name}({joined})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let field = FieldDefinition::parse("title:string").unwrap();
        assert_eq!(field.name, "title");
        assert_eq!(field.field_type, FieldType::String);
        assert!(!field.optional);
        assert!(!field.unique);
        assert!(!field.indexed);
        assert_eq!(field.rust_type(), "String");
        assert_eq!(field.sql_type(), "VARCHAR(255)");
    }

    #[test]
    fn test_parse_optional_field() {
        let field = FieldDefinition::parse("age:integer:optional").unwrap();
        assert_eq!(field.name, "age");
        assert_eq!(field.field_type, FieldType::Integer);
        assert!(field.optional);
        assert_eq!(field.rust_type(), "Option<i32>");
    }

    #[test]
    fn test_parse_unique_field() {
        let field = FieldDefinition::parse("email:string:unique").unwrap();
        assert!(field.unique);
    }

    #[test]
    fn test_parse_indexed_field() {
        let field = FieldDefinition::parse("username:string:indexed").unwrap();
        assert!(field.indexed);
    }

    #[test]
    fn test_parse_multiple_modifiers() {
        let field = FieldDefinition::parse("slug:string:unique:indexed").unwrap();
        assert!(field.unique);
        assert!(field.indexed);
    }

    #[test]
    fn test_parse_reference() {
        let field = FieldDefinition::parse("author:references:User").unwrap();
        assert_eq!(field.name, "author");
        assert_eq!(
            field.field_type,
            FieldType::Reference {
                model: "User".to_string()
            }
        );
        assert_eq!(field.rust_type(), "UserId");
    }

    #[test]
    fn test_parse_belongs_to() {
        let field = FieldDefinition::parse("post:belongs_to:Post").unwrap();
        assert_eq!(
            field.field_type,
            FieldType::Reference {
                model: "Post".to_string()
            }
        );
    }

    #[test]
    fn test_parse_array() {
        let field = FieldDefinition::parse("tags:array:string").unwrap();
        assert_eq!(field.name, "tags");
        assert_eq!(field.rust_type(), "Vec<String>");
    }

    #[test]
    fn test_parse_enum() {
        let field = FieldDefinition::parse("status:enum:Draft,Published,Archived").unwrap();
        assert_eq!(field.name, "status");
        if let FieldType::Enum { name, variants } = &field.field_type {
            assert_eq!(name, "Status");
            assert_eq!(variants, &vec!["Draft", "Published", "Archived"]);
        } else {
            panic!("Expected enum type");
        }
    }

    #[test]
    fn test_parse_all_primitive_types() {
        let test_cases = vec![
            ("name:string", FieldType::String, "String", "VARCHAR(255)"),
            ("bio:text", FieldType::Text, "String", "TEXT"),
            ("age:integer", FieldType::Integer, "i32", "INTEGER"),
            ("count:bigint", FieldType::BigInt, "i64", "BIGINT"),
            ("active:boolean", FieldType::Boolean, "bool", "BOOLEAN"),
            ("price:decimal", FieldType::Decimal, "rust_decimal::Decimal", "DECIMAL(19,4)"),
            ("born:date", FieldType::Date, "chrono::NaiveDate", "DATE"),
            ("created:datetime", FieldType::DateTime, "chrono::NaiveDateTime", "TIMESTAMP"),
        ];

        for (input, expected_type, expected_rust, expected_sql) in test_cases {
            let field = FieldDefinition::parse(input).unwrap();
            assert_eq!(field.field_type, expected_type);
            assert_eq!(field.rust_type(), expected_rust);
            assert_eq!(field.sql_type(), expected_sql);
        }
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(FieldDefinition::parse("invalid").is_err());
        assert!(FieldDefinition::parse(":string").is_err());
        assert!(FieldDefinition::parse("title:").is_err());
    }

    #[test]
    fn test_parse_invalid_field_name() {
        assert!(FieldDefinition::parse("123invalid:string").is_err());
        assert!(FieldDefinition::parse("invalid-name:string").is_err());
    }

    #[test]
    fn test_parse_invalid_type() {
        assert!(FieldDefinition::parse("field:invalid_type").is_err());
    }

    #[test]
    fn test_parse_invalid_modifier() {
        assert!(FieldDefinition::parse("field:string:invalid_modifier").is_err());
    }

    #[test]
    fn test_display() {
        let field = FieldDefinition::parse("email:string:unique:optional").unwrap();
        assert_eq!(field.to_string(), "email:string:optional:unique");
    }
}

# Acton-HTMX Scaffold Templates

This directory contains Handlebars templates used by the `acton-htmx` CLI to generate CRUD scaffolds.

## Template Location

Templates are stored in this repository and downloaded to your local machine on first use:
- **Repository**: `https://github.com/Govcraft/acton-htmx/tree/main/templates/scaffold`
- **Local Cache**: `$XDG_CONFIG_HOME/acton-htmx/templates/scaffold/` (typically `~/.config/acton-htmx/templates/scaffold/`)

## Customization

You can customize these templates by:
1. Locating your local template cache: `~/.config/acton-htmx/templates/scaffold/`
2. Editing the `.hbs` files to match your preferences
3. Running `acton-htmx scaffold crud` - it will use your customized templates

## Template Files

- `model.rs.hbs` - SeaORM entity model with CRUD methods
- `migration.sql.hbs` - PostgreSQL migration with constraints and triggers
- `form.rs.hbs` - Form validation structs
- `handler.rs.hbs` - HTMX-powered request handlers
- `test.rs.hbs` - Integration tests
- `list.html.hbs` - List view Askama template
- `show.html.hbs` - Detail view Askama template
- `form.html.hbs` - Form view Askama template
- `_row.html.hbs` - Row partial Askama template
- `_rows.html.hbs` - Rows partial Askama template

## Template Variables

Templates have access to the following variables:

### Model Information
- `{{model_name}}` - PascalCase model name (e.g., "Post")
- `{{model_snake}}` - snake_case model name (e.g., "post")
- `{{model_plural}}` - Pluralized PascalCase name (e.g., "Posts")
- `{{table_name}}` - Database table name (e.g., "posts")
- `{{route_path}}` - HTTP route path (e.g., "/posts")
- `{{title}}` - Human-readable title (e.g., "Post")
- `{{plural_title}}` - Pluralized title (e.g., "Posts")

### Field Information
- `{{#each fields}}` - Iterate over all fields
  - `{{name}}` - Field name
  - `{{column_name}}` - Database column name
  - `{{rust_type}}` - Rust type (e.g., "String", "i64", "Option<bool>")
  - `{{sql_type}}` - SQL type (e.g., "VARCHAR(255)", "BIGINT")
  - `{{optional}}` - Boolean, true if field is optional
  - `{{unique}}` - Boolean, true if field has unique constraint
  - `{{indexed}}` - Boolean, true if field is indexed
  - `{{validations}}` - Array of validation rules
  - `{{default_value}}` - Default value for tests

### Special Features
- `{{#if has_date_fields}}` - True if any field is a date/datetime
- `{{#if has_decimal}}` - True if any field is decimal
- `{{#if has_uuid}}` - True if any field is UUID
- `{{#if has_enum}}` - True if any field is an enum

### Relations
- `{{#each relations}}` - Foreign key relationships
  - `{{field_name}}` - Relation field name
  - `{{relation_name}}` - PascalCase relation name
  - `{{referenced_table}}` - Referenced table name
  - `{{field_column}}` - Foreign key column name

### Enums
- `{{#each enums}}` - Enum type definitions
  - `{{name}}` - Enum name
  - `{{#each variants}}` - Enum variants

## Example

When you run:
```bash
acton-htmx scaffold crud Post title:string content:text published:boolean
```

The CLI:
1. Downloads templates (if not cached)
2. Parses field definitions
3. Renders each template with the model metadata
4. Writes generated files to your project

## Contributing

To improve these templates:
1. Fork this repository
2. Edit templates in `templates/scaffold/`
3. Test with `acton-htmx scaffold crud`
4. Submit a pull request

## License

MIT License - Same as acton-htmx

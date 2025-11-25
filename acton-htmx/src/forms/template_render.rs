//! Template-based form rendering
//!
//! Renders forms using minijinja templates from the XDG template directory.
//! Templates must be initialized via `acton-htmx templates init` before use.

use minijinja::Value;
use serde::Serialize;

use super::builder::FormBuilder;
use super::error::ValidationErrors;
use super::field::{FieldKind, FormField, InputType, SelectOption};
use super::render::FormRenderOptions;
use crate::template::framework::FrameworkTemplates;

/// Renders forms using minijinja templates
///
/// This renderer uses templates from the XDG template directory,
/// allowing users to customize form HTML structure.
pub struct TemplateFormRenderer<'t> {
    templates: &'t FrameworkTemplates,
    options: FormRenderOptions,
}

impl<'t> TemplateFormRenderer<'t> {
    /// Create a new template-based form renderer
    #[must_use]
    pub fn new(templates: &'t FrameworkTemplates) -> Self {
        Self {
            templates,
            options: FormRenderOptions::default(),
        }
    }

    /// Create a renderer with custom options
    #[must_use]
    pub const fn with_options(templates: &'t FrameworkTemplates, options: FormRenderOptions) -> Self {
        Self { templates, options }
    }

    /// Render a form to HTML string
    ///
    /// # Errors
    ///
    /// Returns error if template rendering fails.
    pub fn render(&self, form: &FormBuilder<'_>) -> Result<String, FormRenderError> {
        // Render all fields first
        let mut fields_html = String::new();
        for field in &form.fields {
            fields_html.push_str(&self.render_field(field, form.errors)?);
        }

        // Build HTMX attributes list
        let hx_attrs = Self::build_htmx_form_attrs(form);

        // Render the form wrapper
        let html = self.templates.render(
            "forms/form.html",
            minijinja::context! {
                action => &form.action,
                method => &form.method,
                id => &form.id,
                class => &form.class,
                enctype => &form.enctype,
                novalidate => form.novalidate,
                csrf_token => &form.csrf_token,
                hx_validate => form.htmx_validate,
                hx_attrs => hx_attrs,
                content => Value::from_safe_string(fields_html),
                submit_label => &form.submit_text,
                submit_class => form.submit_class.as_deref().unwrap_or(&self.options.submit_class),
            },
        )?;

        Ok(html)
    }

    /// Render a single field
    fn render_field(
        &self,
        field: &FormField,
        errors: Option<&ValidationErrors>,
    ) -> Result<String, FormRenderError> {
        let field_errors: Vec<String> = errors
            .map(|e| e.for_field(&field.name))
            .unwrap_or_default()
            .iter()
            .map(|e| e.message.clone())
            .collect();
        let has_errors = !field_errors.is_empty();

        // Hidden fields don't get wrapped
        if matches!(field.kind, FieldKind::Input(InputType::Hidden)) {
            return self.render_input(field, InputType::Hidden, has_errors);
        }

        // Render the field element itself
        let field_html = match &field.kind {
            FieldKind::Input(input_type) => self.render_input(field, *input_type, has_errors)?,
            FieldKind::Textarea { rows, cols } => {
                self.render_textarea(field, *rows, *cols, has_errors)?
            }
            FieldKind::Select { options, multiple } => {
                self.render_select(field, options, *multiple, has_errors)?
            }
            FieldKind::Checkbox { checked } => self.render_checkbox(field, *checked, has_errors)?,
            FieldKind::Radio { options } => self.render_radio(field, options, has_errors)?,
        };

        // Checkbox has label after, others before
        let is_checkbox = matches!(field.kind, FieldKind::Checkbox { .. });

        // Render label if present
        let label_html = if let Some(ref label) = field.label {
            if is_checkbox {
                String::new()
            } else {
                self.render_label(field.effective_id(), label, field.flags.required)?
            }
        } else {
            String::new()
        };

        // Render errors
        let errors_html = if field_errors.is_empty() {
            String::new()
        } else {
            self.render_field_errors(&field_errors)?
        };

        // Wrap in field wrapper
        let html = self.templates.render(
            "forms/field-wrapper.html",
            minijinja::context! {
                wrapper_class => &self.options.group_class,
                error_class => if has_errors { &self.options.input_error_class } else { "" },
                has_error => has_errors,
                label_position => if is_checkbox { "after" } else { "before" },
                label_html => Value::from_safe_string(label_html),
                field_html => Value::from_safe_string(field_html),
                errors => !field_errors.is_empty(),
                errors_html => Value::from_safe_string(errors_html),
                help_text => &field.help_text,
                help_class => &self.options.help_class,
            },
        )?;

        Ok(html)
    }

    /// Render an input field
    fn render_input(
        &self,
        field: &FormField,
        input_type: InputType,
        has_errors: bool,
    ) -> Result<String, FormRenderError> {
        let class = self.build_input_class(field, has_errors);
        let extra_attrs = Self::build_field_attrs(field);

        let html = self.templates.render(
            "forms/input.html",
            minijinja::context! {
                input_type => input_type.as_str(),
                name => &field.name,
                id => field.effective_id(),
                value => &field.value,
                class => class,
                placeholder => &field.placeholder,
                required => field.flags.required,
                disabled => field.flags.disabled,
                readonly => field.flags.readonly,
                autofocus => field.flags.autofocus,
                min => &field.min,
                max => &field.max,
                step => &field.step,
                minlength => field.min_length,
                maxlength => field.max_length,
                pattern => &field.pattern,
                autocomplete => &field.autocomplete,
                // File-specific
                accept => &field.file_attrs.accept,
                multiple => field.file_attrs.multiple,
                data_preview => field.file_attrs.show_preview,
                data_drag_drop => field.file_attrs.drag_drop,
                data_progress_url => &field.file_attrs.progress_endpoint,
                data_max_size => field.file_attrs.max_size_mb,
                extra_attrs => extra_attrs,
            },
        )?;

        Ok(html)
    }

    /// Render a textarea
    fn render_textarea(
        &self,
        field: &FormField,
        rows: Option<u32>,
        cols: Option<u32>,
        has_errors: bool,
    ) -> Result<String, FormRenderError> {
        let class = self.build_input_class(field, has_errors);
        let extra_attrs = Self::build_field_attrs(field);

        let html = self.templates.render(
            "forms/textarea.html",
            minijinja::context! {
                name => &field.name,
                id => field.effective_id(),
                class => class,
                placeholder => &field.placeholder,
                required => field.flags.required,
                disabled => field.flags.disabled,
                readonly => field.flags.readonly,
                rows => rows,
                cols => cols,
                minlength => field.min_length,
                maxlength => field.max_length,
                text_value => field.value.as_deref().unwrap_or(""),
                extra_attrs => extra_attrs,
            },
        )?;

        Ok(html)
    }

    /// Render a select dropdown
    fn render_select(
        &self,
        field: &FormField,
        options: &[SelectOption],
        multiple: bool,
        has_errors: bool,
    ) -> Result<String, FormRenderError> {
        let class = self.build_input_class(field, has_errors);
        let extra_attrs = Self::build_field_attrs(field);

        // Build options with selected state
        let select_options: Vec<SelectOptionCtx> = options
            .iter()
            .map(|opt| SelectOptionCtx {
                value: opt.value.clone(),
                label: opt.label.clone(),
                selected: field.value.as_ref() == Some(&opt.value),
                disabled: opt.disabled,
            })
            .collect();

        let html = self.templates.render(
            "forms/select.html",
            minijinja::context! {
                name => &field.name,
                id => field.effective_id(),
                class => class,
                required => field.flags.required,
                disabled => field.flags.disabled,
                multiple => multiple,
                options => select_options,
                extra_attrs => extra_attrs,
            },
        )?;

        Ok(html)
    }

    /// Render a checkbox
    fn render_checkbox(
        &self,
        field: &FormField,
        checked: bool,
        has_errors: bool,
    ) -> Result<String, FormRenderError> {
        let class = self.build_input_class(field, has_errors);
        let extra_attrs = Self::build_field_attrs(field);

        let html = self.templates.render(
            "forms/checkbox.html",
            minijinja::context! {
                name => &field.name,
                id => field.effective_id(),
                checkbox_value => field.value.as_deref().unwrap_or("true"),
                class => class,
                checked => checked,
                required => field.flags.required,
                disabled => field.flags.disabled,
                label => &field.label,
                label_class => &self.options.label_class,
                extra_attrs => extra_attrs,
            },
        )?;

        Ok(html)
    }

    /// Render radio buttons
    fn render_radio(
        &self,
        field: &FormField,
        options: &[SelectOption],
        has_errors: bool,
    ) -> Result<String, FormRenderError> {
        let class = self.build_input_class(field, has_errors);

        // Build options with IDs
        let radio_options: Vec<RadioOptionCtx> = options
            .iter()
            .enumerate()
            .map(|(i, opt)| RadioOptionCtx {
                id: format!("{}_{}", field.effective_id(), i),
                value: opt.value.clone(),
                label: opt.label.clone(),
                checked: field.value.as_ref() == Some(&opt.value),
                disabled: opt.disabled,
            })
            .collect();

        let html = self.templates.render(
            "forms/radio-group.html",
            minijinja::context! {
                name => &field.name,
                class => class,
                required => field.flags.required,
                disabled => field.flags.disabled,
                options => radio_options,
                radio_wrapper_class => "form-radio",
                label_class => &self.options.label_class,
            },
        )?;

        Ok(html)
    }

    /// Render a label
    fn render_label(
        &self,
        for_id: &str,
        text: &str,
        required: bool,
    ) -> Result<String, FormRenderError> {
        let html = self.templates.render(
            "forms/label.html",
            minijinja::context! {
                for => for_id,
                class => &self.options.label_class,
                text => text,
                required => required,
                required_class => "required",
            },
        )?;

        Ok(html)
    }

    /// Render field errors
    fn render_field_errors(&self, errors: &[String]) -> Result<String, FormRenderError> {
        let html = self.templates.render(
            "validation/field-errors.html",
            minijinja::context! {
                container_class => &self.options.error_class,
                error_class => "error",
                errors => errors,
            },
        )?;

        Ok(html)
    }

    /// Build CSS class for input with error state
    fn build_input_class(&self, field: &FormField, has_errors: bool) -> String {
        let mut classes = vec![self.options.input_class.as_str()];

        if let Some(ref class) = field.class {
            classes.push(class.as_str());
        }
        if has_errors {
            classes.push(self.options.input_error_class.as_str());
        }

        classes.join(" ")
    }

    /// Build extra attributes including HTMX and data attributes
    fn build_field_attrs(field: &FormField) -> Vec<(String, String)> {
        let mut attrs = Vec::new();

        // HTMX attributes
        if let Some(ref url) = field.htmx.get {
            attrs.push(("hx-get".to_string(), url.clone()));
        }
        if let Some(ref url) = field.htmx.post {
            attrs.push(("hx-post".to_string(), url.clone()));
        }
        if let Some(ref url) = field.htmx.put {
            attrs.push(("hx-put".to_string(), url.clone()));
        }
        if let Some(ref url) = field.htmx.delete {
            attrs.push(("hx-delete".to_string(), url.clone()));
        }
        if let Some(ref url) = field.htmx.patch {
            attrs.push(("hx-patch".to_string(), url.clone()));
        }
        if let Some(ref selector) = field.htmx.target {
            attrs.push(("hx-target".to_string(), selector.clone()));
        }
        if let Some(ref strategy) = field.htmx.swap {
            attrs.push(("hx-swap".to_string(), strategy.clone()));
        }
        if let Some(ref trigger) = field.htmx.trigger {
            attrs.push(("hx-trigger".to_string(), trigger.clone()));
        }
        if let Some(ref selector) = field.htmx.indicator {
            attrs.push(("hx-indicator".to_string(), selector.clone()));
        }
        if let Some(ref vals) = field.htmx.vals {
            attrs.push(("hx-vals".to_string(), vals.clone()));
        }
        if field.htmx.validate {
            attrs.push(("hx-validate".to_string(), "true".to_string()));
        }

        // Data attributes
        for (name, value) in &field.data_attrs {
            attrs.push((format!("data-{name}"), value.clone()));
        }

        // Custom attributes
        for (name, value) in &field.custom_attrs {
            attrs.push((name.clone(), value.clone()));
        }

        attrs
    }

    /// Build HTMX form attributes as string list
    fn build_htmx_form_attrs(form: &FormBuilder<'_>) -> Vec<String> {
        let mut attrs = Vec::new();

        if let Some(ref url) = form.htmx.get {
            attrs.push(format!(r#"hx-get="{url}""#));
        }
        if let Some(ref url) = form.htmx.post {
            attrs.push(format!(r#"hx-post="{url}""#));
        }
        if let Some(ref url) = form.htmx.put {
            attrs.push(format!(r#"hx-put="{url}""#));
        }
        if let Some(ref url) = form.htmx.delete {
            attrs.push(format!(r#"hx-delete="{url}""#));
        }
        if let Some(ref url) = form.htmx.patch {
            attrs.push(format!(r#"hx-patch="{url}""#));
        }
        if let Some(ref selector) = form.htmx.target {
            attrs.push(format!(r#"hx-target="{selector}""#));
        }
        if let Some(ref strategy) = form.htmx.swap {
            attrs.push(format!(r#"hx-swap="{strategy}""#));
        }
        if let Some(ref trigger) = form.htmx.trigger {
            attrs.push(format!(r#"hx-trigger="{trigger}""#));
        }
        if let Some(ref selector) = form.htmx.indicator {
            attrs.push(format!(r#"hx-indicator="{selector}""#));
        }
        if let Some(ref url) = form.htmx.push_url {
            attrs.push(format!(r#"hx-push-url="{url}""#));
        }
        if let Some(ref message) = form.htmx.confirm {
            attrs.push(format!(r#"hx-confirm="{message}""#));
        }
        if let Some(ref selector) = form.htmx.disabled_elt {
            attrs.push(format!(r#"hx-disabled-elt="{selector}""#));
        }

        // Custom attributes
        for (name, value) in &form.custom_attrs {
            attrs.push(format!(r#"{name}="{value}""#));
        }

        attrs
    }
}

/// Context for select options in templates
#[derive(Debug, Clone, Serialize)]
struct SelectOptionCtx {
    value: String,
    label: String,
    selected: bool,
    disabled: bool,
}

/// Context for radio options in templates
#[derive(Debug, Clone, Serialize)]
struct RadioOptionCtx {
    id: String,
    value: String,
    label: String,
    checked: bool,
    disabled: bool,
}

/// Errors that can occur during form rendering
#[derive(Debug, thiserror::Error)]
pub enum FormRenderError {
    /// Template rendering failed
    #[error("template error: {0}")]
    TemplateError(#[from] crate::template::framework::FrameworkTemplateError),
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests require templates to be initialized
    // Run `acton-htmx templates init` before running tests

    #[test]
    fn test_template_renderer_creation() {
        // This will fail if templates aren't initialized, which is expected
        let result = FrameworkTemplates::new();
        if result.is_err() {
            // Templates not initialized - skip test
            return;
        }

        let templates = result.unwrap();
        let _renderer = TemplateFormRenderer::new(&templates);
    }
}

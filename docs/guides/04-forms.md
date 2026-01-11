# Form Handling Guide

This guide covers form creation, validation, and HTMX integration in acton-htmx.

## Basic Form

### Form Struct with Validation

```rust
use validator::Validate;
use serde::Deserialize;

#[derive(Debug, Deserialize, Validate)]
pub struct PostForm {
    #[validate(length(min = 1, max = 200))]
    pub title: String,

    #[validate(length(min = 10, max = 10000))]
    pub body: String,

    #[validate(url)]
    pub cover_image: Option<String>,

    pub published: bool,
}
```

### Template

```html
<form hx-post="/posts" hx-target="#main-content">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">

    <div class="field">
        <label for="title">Title</label>
        <input type="text" id="title" name="title" required>
    </div>

    <div class="field">
        <label for="body">Body</label>
        <textarea id="body" name="body" required></textarea>
    </div>

    <div class="field">
        <label>
            <input type="checkbox" name="published" value="true">
            Publish immediately
        </label>
    </div>

    <button type="submit">Create Post</button>
</form>
```

### Handler

```rust
use acton_htmx::prelude::*;
use axum::Form;

pub async fn create_post(
    State(state): State<ActonHtmxState>,
    mut session: SessionExtractor,
    Form(form): Form<PostForm>,
) -> Result<HxRedirect, FormError> {
    // Validate
    form.validate()?;

    // Save to database
    let post_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO posts (title, body, published) VALUES ($1, $2, $3) RETURNING id"
    )
    .bind(&form.title)
    .bind(&form.body)
    .bind(form.published)
    .fetch_one(&state.db_pool)
    .await?;

    session.add_flash(FlashMessage::success("Post created!"));

    Ok(HxRedirect(format!("/posts/{post_id}").parse().unwrap()))
}
```

## Validation

### Built-in Validators

```rust
use validator::Validate;

#[derive(Validate)]
struct UserForm {
    #[validate(email)]
    email: String,

    #[validate(length(min = 8, max = 100))]
    password: String,

    #[validate(url)]
    website: Option<String>,

    #[validate(range(min = 18, max = 120))]
    age: u8,

    #[validate(regex = "PHONE_REGEX")]
    phone: String,
}
```

### Custom Validators

```rust
fn validate_username(username: &str) -> Result<(), validator::ValidationError> {
    if username.starts_with('_') {
        return Err(validator::ValidationError::new("invalid_start"));
    }
    Ok(())
}

#[derive(Validate)]
struct RegisterForm {
    #[validate(custom = "validate_username")]
    #[validate(length(min = 3, max = 20))]
    username: String,
}
```

### Displaying Validation Errors

```rust
#[derive(Template)]
#[template(path = "posts/new.html")]
struct NewPostTemplate {
    errors: Option<ValidationErrors>,
    form: PostForm,
}

async fn show_form(errors: Option<ValidationErrors>) -> impl IntoResponse {
    NewPostTemplate {
        errors,
        form: Default::default(),
    }.render_html()
}

async fn create_post(
    Form(form): Form<PostForm>,
) -> Result<HxRedirect, impl IntoResponse> {
    match form.validate() {
        Ok(_) => {
            save_post(&form).await?;
            Ok(HxRedirect("/posts".parse().unwrap()))
        }
        Err(errors) => {
            Err(NewPostTemplate {
                errors: Some(errors),
                form,
            }.render_html())
        }
    }
}
```

Template:

```html
<div class="field">
    <label for="title">Title</label>
    <input
        type="text"
        id="title"
        name="title"
        value="{{ form.title }}"
        class="{% if errors.title %}error{% endif %}">

    {% if errors %}
        {% if let Some(field_errors) = errors.field_errors().get("title") %}
            <span class="error-message">
                {% for error in field_errors %}
                    {{ error.message }}
                {% endfor %}
            </span>
        {% endif %}
    {% endif %}
</div>
```

## HTMX Form Patterns

### Inline Edit

```html
<div id="post-{{ post.id }}">
    <h2>{{ post.title }}</h2>
    <button hx-get="/posts/{{ post.id }}/edit" hx-target="#post-{{ post.id }}">
        Edit
    </button>
</div>
```

### Form with Loading State

```html
<form hx-post="/posts" hx-indicator="#loading">
    <!-- form fields -->

    <button type="submit">
        <span class="htmx-indicator" id="loading">Saving...</span>
        <span class="default">Save Post</span>
    </button>
</form>
```

### Progressive Enhancement

```html
<form action="/posts" method="post" hx-post="/posts" hx-target="#main-content">
    <!-- Works without JavaScript, enhanced with HTMX -->
</form>
```

## Form Builder API

Use the `FormBuilder` for programmatic form generation:

```rust
use acton_htmx::forms::{FormBuilder, InputType};

let form = FormBuilder::new("post-form")
    .action("/posts")
    .method("post")
    .field("title", InputType::Text)
        .label("Title")
        .required(true)
        .max_length(200)
    .field("body", InputType::Textarea)
        .label("Body")
        .required(true)
        .rows(10)
    .field("published", InputType::Checkbox)
        .label("Publish immediately")
    .submit("Create Post")
    .build();

form.render_html()
```

## File Uploads

```rust
use axum::extract::Multipart;

async fn upload_image(
    mut multipart: Multipart,
) -> Result<HxRedirect, Error> {
    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap().to_string();

        if name == "image" {
            let data = field.bytes().await?;
            let path = save_upload(&data).await?;

            return Ok(HxRedirect(format!("/images/{path}").parse()?));
        }
    }

    Err(Error::MissingFile)
}
```

Template:

```html
<form hx-post="/upload" hx-encoding="multipart/form-data">
    <input type="file" name="image" accept="image/*">
    <button type="submit">Upload</button>
</form>
```

## Next Steps

- **[Deployment Guide](05-deployment.md)** - Deploy to production
- **[Examples](../examples/)** - Complete form examples

## Reference

- [validator crate](https://docs.rs/validator)
- [HTMX Forms](https://htmx.org/examples/)
- [acton-htmx Forms API](../../target/doc/acton_htmx/forms/)

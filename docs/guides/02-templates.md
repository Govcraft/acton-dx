# Template Integration Guide

This guide covers Askama template integration with HTMX in acton-htmx.

## Overview

acton-htmx uses [Askama](https://djc.github.io/askama/) for compile-time templating with these benefits:

- **Type-safe**: Templates are checked at compile time
- **Fast**: No runtime template parsing
- **HTMX-aware**: Automatic partial/full page rendering
- **Rust integration**: Use Rust expressions in templates

## Basic Template

### Define a Template

```rust
use askama::Template;
use acton_htmx::prelude::*;

#[derive(Template)]
#[template(path = "posts/index.html")]
struct PostsIndexTemplate {
    posts: Vec<Post>,
    user: Option<User>,
}
```

### Template File

```html
<!-- templates/posts/index.html -->
{% extends "layouts/app.html" %}

{% block content %}
<div id="main-content">
    <h1>Posts</h1>

    <div id="posts-list">
        {% for post in posts %}
            {% include "posts/_post.html" %}
        {% endfor %}
    </div>
</div>
{% endblock %}
```

### Render in Handler

```rust
async fn index(
    State(state): State<ActonHtmxState>,
    HxRequest(is_htmx): HxRequest,
    auth: OptionalAuth<User>,
) -> impl axum::response::IntoResponse {
    let posts = load_posts(&state.db_pool).await?;

    let template = PostsIndexTemplate {
        posts,
        user: auth.user,
    };

    template.render_htmx(is_htmx)
}
```

## Template Layouts

### Base Layout

```html
<!-- templates/layouts/base.html -->
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}My App{% endblock %}</title>
    <link rel="stylesheet" href="/static/css/app.css">
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>
</head>
<body>
    {% block body %}{% endblock %}
</body>
</html>
```

### App Layout with Navigation

```html
<!-- templates/layouts/app.html -->
{% extends "layouts/base.html" %}

{% block body %}
<div class="container">
    {% include "partials/nav.html" %}

    <div id="flash-messages">
        {% for message in flash_messages %}
            {% include "partials/flash.html" %}
        {% endfor %}
    </div>

    <main>
        {% block content %}{% endblock %}
    </main>

    <footer>
        <p>&copy; 2025 My App</p>
    </footer>
</div>
{% endblock %}
```

## Automatic Partial Rendering

The `HxTemplate` trait automatically renders partials for HTMX requests:

```rust
#[derive(Template)]
#[template(path = "posts/show.html")]
struct PostShowTemplate {
    post: Post,
}

async fn show(
    Path(id): Path<i64>,
    HxRequest(is_htmx): HxRequest,
) -> impl axum::response::IntoResponse {
    let post = load_post(id).await;

    // Automatically renders:
    // - Full page (with layout) if browser request
    // - Just #main-content if HTMX request
    PostShowTemplate { post }.render_htmx(is_htmx)
}
```

### How It Works

Templates must have a `<div id="main-content">` wrapper:

```html
<!-- templates/posts/show.html -->
{% extends "layouts/app.html" %}

{% block content %}
<div id="main-content">
    <article class="post">
        <h1>{{ post.title }}</h1>
        <p>{{ post.body }}</p>
    </article>
</div>
{% endblock %}
```

HTMX requests extract and return just the `#main-content` div.

## Inline Templates

For simple templates, use inline source:

```rust
#[derive(Template)]
#[template(source = r#"
<div class="post">
    <h3>{{ title }}</h3>
    <p>{{ body }}</p>
</div>
"#, ext = "html")]
struct PostPartial {
    title: String,
    body: String,
}

async fn create_post(form: Form<PostForm>) -> impl axum::response::IntoResponse {
    let post = save_post(form).await;

    PostPartial {
        title: post.title,
        body: post.body,
    }.render_html()
}
```

## Template Helpers

### CSRF Token

Include CSRF token in forms:

```html
<form method="post" action="/posts">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">

    <!-- form fields -->
</form>
```

Pass it from your handler:

```rust
#[derive(Template)]
#[template(path = "posts/new.html")]
struct NewPostTemplate {
    csrf_token: String,
}

async fn new(csrf: CsrfToken) -> impl axum::response::IntoResponse {
    NewPostTemplate {
        csrf_token: csrf.token().to_string(),
    }.render_html()
}
```

### Flash Messages

Display flash messages from session:

```rust
#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    flash_messages: Vec<FlashMessage>,
}

async fn dashboard(mut session: SessionExtractor) -> impl axum::response::IntoResponse {
    let flash_messages = session.take_flashes();

    DashboardTemplate { flash_messages }.render_html()
}
```

```html
<!-- templates/partials/flash.html -->
{% if flash_messages %}
<div id="flash-messages">
    {% for message in flash_messages %}
    <div class="alert alert-{{ message.level }}">
        {{ message.message }}
    </div>
    {% endfor %}
</div>
{% endif %}
```

### Conditional Content

Show content based on conditions:

```html
{% if user %}
    <p>Welcome, {{ user.email }}!</p>
    <a href="/logout">Logout</a>
{% else %}
    <a href="/login">Login</a>
    <a href="/register">Register</a>
{% endif %}
```

### Loops

Iterate over collections:

```html
<ul>
{% for post in posts %}
    <li>
        <a href="/posts/{{ post.id }}">{{ post.title }}</a>
        <span>by {{ post.author }}</span>
    </li>
{% endfor %}
</ul>

{% if posts.is_empty() %}
    <p>No posts yet.</p>
{% endif %}
```

### Filters

Apply Rust functions to values:

```html
<!-- Uppercase -->
<h1>{{ title|upper }}</h1>

<!-- Truncate -->
<p>{{ body|truncate(100) }}</p>

<!-- Format date -->
<time>{{ created_at|date("%Y-%m-%d") }}</time>

<!-- Custom filter -->
<div>{{ markdown_content|markdown|safe }}</div>
```

Define custom filters:

```rust
use askama::Template;

pub fn markdown(s: &str) -> askama::Result<String> {
    // Render markdown to HTML
    Ok(markdown::to_html(s))
}
```

## Partials

### Extract Reusable Components

```html
<!-- templates/posts/_post.html -->
<article class="post" id="post-{{ post.id }}">
    <h2>
        <a href="/posts/{{ post.id }}">{{ post.title }}</a>
    </h2>
    <p>{{ post.excerpt }}</p>
    <footer>
        <span>{{ post.author }}</span>
        <time>{{ post.created_at }}</time>
    </footer>
</article>
```

### Include Partials

```html
<!-- templates/posts/index.html -->
<div id="posts-list">
    {% for post in posts %}
        {% include "posts/_post.html" %}
    {% endfor %}
</div>
```

## HTMX Patterns

### Inline Editing

```html
<div id="post-{{ post.id }}" class="post">
    <h2>{{ post.title }}</h2>

    <button
        hx-get="/posts/{{ post.id }}/edit"
        hx-target="#post-{{ post.id }}"
        hx-swap="outerHTML">
        Edit
    </button>
</div>
```

Edit form template:

```html
<!-- templates/posts/_edit_form.html -->
<form
    id="post-{{ post.id }}"
    hx-put="/posts/{{ post.id }}"
    hx-target="#post-{{ post.id }}"
    hx-swap="outerHTML">

    <input type="text" name="title" value="{{ post.title }}">
    <textarea name="body">{{ post.body }}</textarea>

    <button type="submit">Save</button>
    <button
        type="button"
        hx-get="/posts/{{ post.id }}"
        hx-target="#post-{{ post.id }}"
        hx-swap="outerHTML">
        Cancel
    </button>
</form>
```

### Infinite Scroll

```html
<div id="posts-container">
    {% for post in posts %}
        {% include "posts/_post.html" %}
    {% endfor %}

    {% if has_more %}
    <div
        hx-get="/posts?page={{ next_page }}"
        hx-trigger="revealed"
        hx-swap="afterend">
        <p>Loading more...</p>
    </div>
    {% endif %}
</div>
```

### Search with Debounce

```html
<input
    type="search"
    name="q"
    placeholder="Search posts..."
    hx-get="/posts/search"
    hx-trigger="keyup changed delay:500ms"
    hx-target="#search-results">

<div id="search-results">
    {% for post in posts %}
        {% include "posts/_post.html" %}
    {% endfor %}
</div>
```

### Delete Confirmation

```html
<button
    hx-delete="/posts/{{ post.id }}"
    hx-confirm="Are you sure you want to delete this post?"
    hx-target="#post-{{ post.id }}"
    hx-swap="outerHTML swap:1s">
    Delete
</button>
```

### Active Search (Live Results)

```html
<form>
    <input
        type="search"
        name="q"
        placeholder="Search..."
        hx-get="/search"
        hx-trigger="input changed delay:300ms, search"
        hx-target="#results"
        hx-indicator="#loading">

    <span id="loading" class="htmx-indicator">Searching...</span>
</form>

<div id="results">
    <!-- Search results appear here -->
</div>
```

## Template Organization

### Recommended Structure

```
templates/
├── layouts/
│   ├── base.html           # Base HTML structure
│   └── app.html            # App layout with nav/footer
├── partials/
│   ├── nav.html            # Navigation component
│   ├── flash.html          # Flash messages
│   └── pagination.html     # Pagination controls
├── posts/
│   ├── index.html          # Post list page
│   ├── show.html           # Post detail page
│   ├── new.html            # New post form
│   ├── edit.html           # Edit post form
│   └── _post.html          # Post partial
├── auth/
│   ├── login.html          # Login page
│   └── register.html       # Registration page
└── home.html               # Homepage
```

### Naming Conventions

- **Pages**: `index.html`, `show.html`, `new.html`, `edit.html`
- **Partials**: Prefix with `_` (e.g., `_post.html`, `_comment.html`)
- **Layouts**: Store in `layouts/` directory
- **Components**: Store in `partials/` directory

## Error Handling

### Display Validation Errors

```rust
#[derive(Template)]
#[template(path = "posts/new.html")]
struct NewPostTemplate {
    errors: Option<ValidationErrors>,
    form: PostForm,
}

async fn create(
    Form(form): Form<PostForm>,
) -> Result<HxRedirect, impl axum::response::IntoResponse> {
    match form.validate() {
        Ok(_) => {
            save_post(form).await;
            Ok(HxRedirect("/posts".parse().unwrap()))
        }
        Err(errors) => {
            let template = NewPostTemplate {
                errors: Some(errors),
                form,
            };
            Err(template.render_html())
        }
    }
}
```

```html
<!-- templates/posts/new.html -->
<form method="post" hx-post="/posts" hx-target="this">
    <div class="field">
        <label>Title</label>
        <input type="text" name="title" value="{{ form.title }}">
        {% if errors %}
            {% if errors.title %}
                <span class="error">{{ errors.title }}</span>
            {% endif %}
        {% endif %}
    </div>

    <button type="submit">Create Post</button>
</form>
```

## Best Practices

### 1. Use Layouts Consistently

Define a base layout and extend it:

```html
{% extends "layouts/app.html" %}
```

### 2. Extract Reusable Partials

Don't repeat markup—use includes:

```html
{% include "posts/_post.html" %}
```

### 3. Keep Templates Simple

Move complex logic to handlers:

```rust
// Good
#[derive(Template)]
struct PostsTemplate {
    posts: Vec<Post>,
    total_count: usize,
}

// Bad - too much logic in template
{% set total = posts.len() + archived_posts.len() %}
```

### 4. Use HTMX Attributes

Make forms and links HTMX-aware:

```html
<!-- Good -->
<form hx-post="/posts" hx-target="#main-content">

<!-- Also good for progressive enhancement -->
<form action="/posts" method="post" hx-post="/posts" hx-target="#main-content">
```

### 5. Wrap Main Content

Always wrap content in `#main-content` for automatic partials:

```html
<div id="main-content">
    <!-- Your content -->
</div>
```

## Template Testing

Test template rendering:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_template_renders() {
        let post = Post {
            id: 1,
            title: "Test".to_string(),
            body: "Body".to_string(),
        };

        let template = PostShowTemplate { post };
        let html = template.render().unwrap();

        assert!(html.contains("Test"));
        assert!(html.contains("Body"));
    }
}
```

## Next Steps

- **[Authentication Guide](03-authentication.md)** - Add login and registration
- **[Form Handling](04-forms.md)** - Build validated forms
- **[Examples](../examples/)** - See complete working examples

## Reference

- [Askama Documentation](https://djc.github.io/askama/)
- [HTMX Documentation](https://htmx.org/docs/)
- [acton-htmx Template API](../../target/doc/acton_htmx/template/)

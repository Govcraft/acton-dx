# Example: Blog with CRUD Operations

This example demonstrates a complete blog application with Create, Read, Update, Delete operations using HTMX.

## Features

- List all blog posts
- View individual post
- Create new post (authenticated)
- Edit existing post (authenticated)
- Delete post with confirmation (authenticated)
- Inline editing
- Flash messages
- CSRF protection
- Pagination

## Database Schema

```sql
-- migrations/001_create_posts.sql
CREATE TABLE posts (
    id SERIAL PRIMARY KEY,
    title VARCHAR(200) NOT NULL,
    slug VARCHAR(200) UNIQUE NOT NULL,
    body TEXT NOT NULL,
    excerpt VARCHAR(500),
    author_id INTEGER NOT NULL REFERENCES users(id),
    published BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_posts_published ON posts(published);
CREATE INDEX idx_posts_author ON posts(author_id);
CREATE INDEX idx_posts_slug ON posts(slug);
```

## Models

```rust
use sqlx::FromRow;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub excerpt: Option<String>,
    pub author_id: i64,
    pub published: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePostForm {
    #[validate(length(min = 1, max = 200))]
    pub title: String,

    #[validate(length(min = 10, max = 50000))]
    pub body: String,

    #[validate(length(max = 500))]
    pub excerpt: Option<String>,

    pub published: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePostForm {
    #[validate(length(min = 1, max = 200))]
    pub title: String,

    #[validate(length(min = 10, max = 50000))]
    pub body: String,

    #[validate(length(max = 500))]
    pub excerpt: Option<String>,

    pub published: bool,
}
```

## Handlers

### List Posts

```rust
use acton_htmx::prelude::*;
use askama::Template;

#[derive(Template)]
#[template(path = "posts/index.html")]
struct PostsIndexTemplate {
    posts: Vec<Post>,
    user: Option<User>,
}

pub async fn index(
    State(state): State<ActonHtmxState>,
    HxRequest(is_htmx): HxRequest,
    OptionalAuth(user): OptionalAuth<User>,
) -> Result<impl axum::response::IntoResponse, PostError> {
    let posts = sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, slug, body, excerpt, author_id, published,
               created_at, updated_at
        FROM posts
        WHERE published = true
        ORDER BY created_at DESC
        LIMIT 20
        "#
    )
    .fetch_all(&state.db_pool)
    .await?;

    Ok(PostsIndexTemplate { posts, user }.render_htmx(is_htmx))
}
```

### Show Post

```rust
#[derive(Template)]
#[template(path = "posts/show.html")]
struct PostShowTemplate {
    post: Post,
    author: User,
    user: Option<User>,
}

pub async fn show(
    State(state): State<ActonHtmxState>,
    HxRequest(is_htmx): HxRequest,
    OptionalAuth(user): OptionalAuth<User>,
    Path(slug): Path<String>,
) -> Result<impl axum::response::IntoResponse, PostError> {
    let post = sqlx::query_as!(
        Post,
        "SELECT * FROM posts WHERE slug = $1 AND published = true",
        slug
    )
    .fetch_optional(&state.db_pool)
    .await?
    .ok_or(PostError::NotFound)?;

    let author = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE id = $1",
        post.author_id
    )
    .fetch_one(&state.db_pool)
    .await?;

    Ok(PostShowTemplate {
        post,
        author,
        user,
    }
    .render_htmx(is_htmx))
}
```

### Create Post

```rust
#[derive(Template)]
#[template(path = "posts/new.html")]
struct NewPostTemplate {
    user: User,
    csrf_token: String,
    errors: Option<ValidationErrors>,
    form: Option<CreatePostForm>,
}

pub async fn new(
    Authenticated(user): Authenticated<User>,
    csrf: CsrfToken,
) -> impl axum::response::IntoResponse {
    NewPostTemplate {
        user,
        csrf_token: csrf.token().to_string(),
        errors: None,
        form: None,
    }
    .render_html()
}

pub async fn create(
    State(state): State<ActonHtmxState>,
    Authenticated(user): Authenticated<User>,
    mut session: SessionExtractor,
    Form(form): Form<CreatePostForm>,
) -> Result<HxRedirect, impl axum::response::IntoResponse> {
    // Validate form
    if let Err(errors) = form.validate() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            NewPostTemplate {
                user,
                csrf_token: String::new(),
                errors: Some(errors),
                form: Some(form),
            }
            .render_html(),
        ));
    }

    // Generate slug from title
    let slug = slugify(&form.title);

    // Insert post
    let post_id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO posts (title, slug, body, excerpt, author_id, published)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#
    )
    .bind(&form.title)
    .bind(&slug)
    .bind(&form.body)
    .bind(&form.excerpt)
    .bind(user.id)
    .bind(form.published)
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            NewPostTemplate {
                user,
                csrf_token: String::new(),
                errors: None,
                form: Some(form),
            }
            .render_html(),
        )
    })?;

    session.add_flash(FlashMessage::success("Post created successfully!"));

    Ok(HxRedirect(format!("/posts/{slug}").parse().unwrap()))
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
```

### Edit Post

```rust
#[derive(Template)]
#[template(path = "posts/edit.html")]
struct EditPostTemplate {
    post: Post,
    user: User,
    csrf_token: String,
    errors: Option<ValidationErrors>,
}

pub async fn edit(
    State(state): State<ActonHtmxState>,
    Authenticated(user): Authenticated<User>,
    csrf: CsrfToken,
    Path(id): Path<i64>,
) -> Result<impl axum::response::IntoResponse, PostError> {
    let post = sqlx::query_as!(
        Post,
        "SELECT * FROM posts WHERE id = $1 AND author_id = $2",
        id,
        user.id
    )
    .fetch_optional(&state.db_pool)
    .await?
    .ok_or(PostError::NotFound)?;

    Ok(EditPostTemplate {
        post,
        user,
        csrf_token: csrf.token().to_string(),
        errors: None,
    }
    .render_html())
}

pub async fn update(
    State(state): State<ActonHtmxState>,
    Authenticated(user): Authenticated<User>,
    mut session: SessionExtractor,
    Path(id): Path<i64>,
    Form(form): Form<UpdatePostForm>,
) -> Result<HxRedirect, impl axum::response::IntoResponse> {
    // Validate form
    if let Err(errors) = form.validate() {
        let post = load_post(&state, id).await?;
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            EditPostTemplate {
                post,
                user,
                csrf_token: String::new(),
                errors: Some(errors),
            }
            .render_html(),
        ));
    }

    // Update post
    let rows = sqlx::query!(
        r#"
        UPDATE posts
        SET title = $1, body = $2, excerpt = $3, published = $4, updated_at = NOW()
        WHERE id = $5 AND author_id = $6
        "#,
        form.title,
        form.body,
        form.excerpt,
        form.published,
        id,
        user.id
    )
    .execute(&state.db_pool)
    .await
    .map_err(|_| {
        let post = Post { /* ... */ };
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            EditPostTemplate {
                post,
                user,
                csrf_token: String::new(),
                errors: None,
            }
            .render_html(),
        )
    })?;

    if rows.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Html("Post not found").into_response(),
        ));
    }

    session.add_flash(FlashMessage::success("Post updated successfully!"));

    Ok(HxRedirect(format!("/posts/{id}").parse().unwrap()))
}
```

### Delete Post

```rust
pub async fn delete(
    State(state): State<ActonHtmxState>,
    Authenticated(user): Authenticated<User>,
    mut session: SessionExtractor,
    Path(id): Path<i64>,
) -> Result<HxRedirect, PostError> {
    let rows = sqlx::query!(
        "DELETE FROM posts WHERE id = $1 AND author_id = $2",
        id,
        user.id
    )
    .execute(&state.db_pool)
    .await?;

    if rows.rows_affected() == 0 {
        return Err(PostError::NotFound);
    }

    session.add_flash(FlashMessage::success("Post deleted successfully!"));

    Ok(HxRedirect("/posts".parse().unwrap()))
}
```

## Templates

### List View (templates/posts/index.html)

```html
{% extends "layouts/app.html" %}

{% block content %}
<div id="main-content">
    <div class="header">
        <h1>Blog Posts</h1>
        {% if user %}
        <a href="/posts/new" class="btn btn-primary">New Post</a>
        {% endif %}
    </div>

    <div id="posts-list">
        {% for post in posts %}
        {% include "posts/_post.html" %}
        {% endfor %}
    </div>
</div>
{% endblock %}
```

### Post Partial (templates/posts/_post.html)

```html
<article class="post" id="post-{{ post.id }}">
    <h2>
        <a href="/posts/{{ post.slug }}">{{ post.title }}</a>
    </h2>

    {% if post.excerpt %}
    <p class="excerpt">{{ post.excerpt }}</p>
    {% endif %}

    <footer class="post-meta">
        <time>{{ post.created_at|date("%B %d, %Y") }}</time>

        {% if user and user.id == post.author_id %}
        <div class="actions">
            <a href="/posts/{{ post.id }}/edit" class="btn btn-sm">Edit</a>

            <button
                hx-delete="/posts/{{ post.id }}"
                hx-confirm="Are you sure you want to delete this post?"
                hx-target="#post-{{ post.id }}"
                hx-swap="outerHTML swap:1s"
                class="btn btn-sm btn-danger">
                Delete
            </button>
        </div>
        {% endif %}
    </footer>
</article>
```

### Show View (templates/posts/show.html)

```html
{% extends "layouts/app.html" %}

{% block content %}
<div id="main-content">
    <article class="post-full">
        <header>
            <h1>{{ post.title }}</h1>
            <div class="post-meta">
                <span>By {{ author.email }}</span>
                <time>{{ post.created_at|date("%B %d, %Y") }}</time>
            </div>
        </header>

        <div class="post-body">
            {{ post.body|markdown|safe }}
        </div>

        {% if user and user.id == post.author_id %}
        <footer class="post-actions">
            <a href="/posts/{{ post.id }}/edit" class="btn">Edit Post</a>

            <button
                hx-delete="/posts/{{ post.id }}"
                hx-confirm="Are you sure you want to delete this post?"
                class="btn btn-danger">
                Delete Post
            </button>
        </footer>
        {% endif %}
    </article>

    <a href="/posts" class="back-link">← Back to all posts</a>
</div>
{% endblock %}
```

### Form View (templates/posts/new.html)

```html
{% extends "layouts/app.html" %}

{% block content %}
<div id="main-content">
    <h1>New Post</h1>

    <form hx-post="/posts" hx-target="#main-content">
        <input type="hidden" name="csrf_token" value="{{ csrf_token }}">

        <div class="field {% if errors and errors.title %}error{% endif %}">
            <label for="title">Title</label>
            <input
                type="text"
                id="title"
                name="title"
                value="{% if form %}{{ form.title }}{% endif %}"
                required>
            {% if errors and errors.title %}
            <span class="error-message">{{ errors.title }}</span>
            {% endif %}
        </div>

        <div class="field {% if errors and errors.body %}error{% endif %}">
            <label for="body">Body</label>
            <textarea
                id="body"
                name="body"
                rows="20"
                required>{% if form %}{{ form.body }}{% endif %}</textarea>
            {% if errors and errors.body %}
            <span class="error-message">{{ errors.body }}</span>
            {% endif %}
        </div>

        <div class="field">
            <label for="excerpt">Excerpt (optional)</label>
            <textarea
                id="excerpt"
                name="excerpt"
                rows="3">{% if form %}{{ form.excerpt }}{% endif %}</textarea>
        </div>

        <div class="field checkbox">
            <label>
                <input
                    type="checkbox"
                    name="published"
                    value="true"
                    {% if form and form.published %}checked{% endif %}>
                Publish immediately
            </label>
        </div>

        <div class="actions">
            <button type="submit" class="btn btn-primary">Create Post</button>
            <a href="/posts" class="btn">Cancel</a>
        </div>
    </form>
</div>
{% endblock %}
```

## Routes

```rust
use axum::{routing::{get, post, put, delete}, Router};

pub fn routes() -> Router<ActonHtmxState> {
    Router::new()
        .route("/posts", get(posts::index))
        .route("/posts/new", get(posts::new))
        .route("/posts", post(posts::create))
        .route("/posts/:slug", get(posts::show))
        .route("/posts/:id/edit", get(posts::edit))
        .route("/posts/:id", put(posts::update))
        .route("/posts/:id", delete(posts::delete))
}
```

## Running the Example

```bash
# Create a new project
acton-htmx new blog-example
cd blog-example

# Set up database
createdb blog_dev
acton-htmx db migrate

# Start development server
acton-htmx dev
```

Visit http://localhost:3000/posts to see the blog!

## Next Steps

- Add comments functionality
- Implement pagination
- Add search
- Add tags/categories
- Add markdown preview
- Add image uploads

## Complete Code

The complete working example is available in the `examples/blog-crud` directory of the repository.

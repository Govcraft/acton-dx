# File Uploads

**Guide to implementing secure file uploads with validation, processing, and serving**

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Upload Form Helpers](#upload-form-helpers)
- [File Validation](#file-validation)
- [Image Processing](#image-processing)
- [File Serving](#file-serving)
- [Security Best Practices](#security-best-practices)
- [Complete Example](#complete-example)

## Overview

acton-htmx provides a comprehensive file upload system with:

- **Form helpers** for building upload forms with drag-and-drop, previews, and progress tracking
- **Extractors** for handling single and multiple file uploads
- **Validation** with magic number detection (never trust client headers!)
- **Image processing** for thumbnails, resizing, and EXIF stripping
- **Virus scanning** integration (pluggable backends)
- **Upload policies** for role-based restrictions and quotas
- **File serving** with range requests, caching, and access control

## Quick Start

### 1. Create an Upload Form

```rust
use acton_htmx::forms::FormBuilder;

let upload_form = FormBuilder::new("/upload", "POST")
    .file("avatar")
        .label("Profile Picture")
        .accept("image/png,image/jpeg,image/gif")
        .max_size_mb(5)
        .show_preview()
        .required()
        .done()
    .submit("Upload")
    .build();
```

### 2. Handle the Upload

```rust
use acton_htmx::extractors::FileUpload;
use acton_htmx::storage::{FileStorage, LocalFileStorage, MimeValidator};
use axum::{extract::State, response::IntoResponse};
use std::sync::Arc;

async fn upload_avatar(
    State(storage): State<Arc<LocalFileStorage>>,
    FileUpload(file): FileUpload,
) -> Result<impl IntoResponse, String> {
    // Validate MIME type using magic numbers (security-first!)
    MimeValidator::validate_against_magic(&file, true)
        .map_err(|e| e.to_string())?;

    MimeValidator::validate_mime_type(&file, &["image/png", "image/jpeg", "image/gif"])
        .map_err(|e| e.to_string())?;

    // Validate size
    file.validate_size(5 * 1024 * 1024) // 5MB
        .map_err(|e| e.to_string())?;

    // Store the file
    let stored = storage.store(file).await
        .map_err(|e| e.to_string())?;

    Ok(format!("Uploaded file: {}", stored.id))
}
```

### 3. Serve the File

```rust
use acton_htmx::middleware::serve_file;
use axum::{Router, routing::get};

let app = Router::new()
    .route("/files/:id", get(serve_file::<LocalFileStorage>))
    .with_state(storage);
```

## Upload Form Helpers

### Basic File Input

```rust
FormBuilder::new("/upload", "POST")
    .file("avatar")
        .label("Avatar")
        .required()
        .done()
    .build()
```

Generated HTML:
```html
<form action="/upload" method="POST" enctype="multipart/form-data">
  <div class="form-group">
    <label for="avatar" class="form-label">Avatar</label>
    <input type="file" name="avatar" id="avatar" class="form-input" required>
  </div>
</form>
```

### File Type Restrictions

```rust
// MIME types
.file("image")
    .accept("image/png,image/jpeg,image/gif")
    .done()

// File extensions
.file("document")
    .accept(".pdf,.doc,.docx")
    .done()
```

### Multiple Files

```rust
.file("attachments")
    .label("Attachments")
    .multiple()
    .max_size_mb(10)
    .done()
```

### Preview and Drag-Drop

```rust
.file("photo")
    .label("Photo")
    .accept("image/*")
    .show_preview()
    .drag_drop()
    .done()
```

Attributes added:
- `data-preview="true"` - Enable client-side image preview
- `data-drag-drop="true"` - Enable drag-and-drop zone styling

### Progress Tracking (SSE)

```rust
.file("large_file")
    .label("Large File")
    .progress_endpoint("/upload/progress")
    .done()
```

Adds `data-progress-endpoint="/upload/progress"` for SSE integration.

## File Validation

### MIME Type Validation

**Always validate using magic numbers** to prevent MIME type spoofing:

```rust
use acton_htmx::storage::MimeValidator;

// Strict validation (magic numbers only)
MimeValidator::validate_against_magic(&file, true)?;

// Permissive (check both magic numbers and header)
MimeValidator::validate_against_magic(&file, false)?;

// Validate specific MIME types
MimeValidator::validate_mime_type(&file, &["image/png", "image/jpeg"])?;

// Helper methods
if MimeValidator::is_image(&file)? {
    // Process image
}
```

### Size Validation

```rust
// Validate size in bytes
file.validate_size(5 * 1024 * 1024)?; // 5MB

// Check size
if file.size() > 10_000_000 {
    return Err("File too large");
}
```

### Upload Policies

```rust
use acton_htmx::storage::{UploadPolicy, PolicyBuilder};

let policy = PolicyBuilder::new()
    .max_file_size(10 * 1024 * 1024) // 10MB
    .allowed_mime_types(vec!["image/png".into(), "image/jpeg".into()])
    .max_files_per_upload(5)
    .quota_bytes_per_user(100 * 1024 * 1024) // 100MB total
    .build();

// Check if upload allowed
if policy.check_file(&file).is_ok() {
    // Process upload
}
```

## Image Processing

### Thumbnail Generation

```rust
use acton_htmx::storage::ImageProcessor;

let thumbnail = ImageProcessor::create_thumbnail(&file, 200, 200)?;
storage.store(thumbnail).await?;
```

### Resize Images

```rust
let resized = ImageProcessor::resize(&file, 800, 600)?;
```

### Strip EXIF Metadata

```rust
// Remove EXIF data for privacy
let clean_image = ImageProcessor::strip_exif(&file)?;
```

### Format Conversion

```rust
let png = ImageProcessor::convert_format(&file, "png")?;
let webp = ImageProcessor::convert_format(&file, "webp")?;
```

## File Serving

### Basic File Serving

```rust
use acton_htmx::middleware::serve_file;
use axum::{Router, routing::get};
use std::sync::Arc;

let storage = Arc::new(LocalFileStorage::new(PathBuf::from("/var/uploads"))?);

let app = Router::new()
    .route("/files/:id", get(serve_file::<LocalFileStorage>))
    .with_state(storage);
```

### With Access Control

```rust
use acton_htmx::middleware::{FileServingMiddleware, FileAccessControl};

let access_control: FileAccessControl = Arc::new(|user_id, file_id| {
    Box::pin(async move {
        // Check if user owns the file or is admin
        let user = get_user(user_id).await?;
        let file = get_file_metadata(&file_id).await?;

        Ok(user.id == file.owner_id || user.is_admin)
    })
});

let middleware = FileServingMiddleware::new(storage)
    .with_access_control(access_control)
    .with_cache_max_age(86400) // 1 day
    .with_cdn_headers();
```

### Features

The file serving middleware provides:

1. **Range Requests** - Streaming and resumable downloads
   ```
   Range: bytes=0-1023
   Range: bytes=500-
   Range: bytes=-500
   ```

2. **Caching** - ETag and Last-Modified headers
   ```
   ETag: "file-id-12345-67890"
   Cache-Control: public, max-age=86400
   Last-Modified: Mon, 21 Nov 2025 10:00:00 GMT
   ```

3. **Conditional Requests** - 304 Not Modified
   ```
   If-None-Match: "file-id-12345-67890"
   If-Modified-Since: Mon, 21 Nov 2025 10:00:00 GMT
   ```

## Security Best Practices

### 1. Never Trust Client Headers

**❌ Wrong:**
```rust
// Don't trust Content-Type header!
if file.content_type == "image/png" {
    store(file).await
}
```

**✅ Correct:**
```rust
// Validate using magic numbers
MimeValidator::validate_against_magic(&file, true)?;
```

### 2. Always Validate Size

```rust
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB

if file.size() > MAX_FILE_SIZE {
    return Err("File too large");
}
```

### 3. Use Upload Policies

```rust
let policy = PolicyBuilder::new()
    .max_file_size(10 * 1024 * 1024)
    .allowed_mime_types(vec!["image/png".into(), "image/jpeg".into()])
    .build();

policy.check_file(&file)?;
```

### 4. Store Files Outside Webroot

```rust
// ✅ Good - files served through controlled endpoint
let storage = LocalFileStorage::new(PathBuf::from("/var/app/uploads"))?;

// ❌ Bad - files directly accessible
let storage = LocalFileStorage::new(PathBuf::from("/var/www/public/uploads"))?;
```

### 5. Generate Unique IDs

acton-htmx generates UUIDs automatically:
```rust
let stored = storage.store(file).await?;
println!("File ID: {}", stored.id); // UUID v4
```

### 6. Strip Metadata

```rust
// Remove EXIF data that might contain location or personal info
let clean = ImageProcessor::strip_exif(&file)?;
```

### 7. Virus Scanning (Optional)

```rust
use acton_htmx::storage::ClamAvScanner;

let scanner = ClamAvScanner::new()?;
let result = scanner.scan(&file).await?;

if !result.is_clean {
    return Err("File contains malware");
}
```

## Complete Example

### Handler with Full Validation

```rust
use acton_htmx::extractors::FileUpload;
use acton_htmx::storage::{
    FileStorage, LocalFileStorage, MimeValidator, ImageProcessor,
    UploadPolicy, PolicyBuilder
};
use axum::{extract::State, response::IntoResponse, http::StatusCode};
use std::sync::Arc;

async fn upload_profile_picture(
    State(storage): State<Arc<LocalFileStorage>>,
    FileUpload(file): FileUpload,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // 1. Validate using upload policy
    let policy = PolicyBuilder::new()
        .max_file_size(5 * 1024 * 1024) // 5MB
        .allowed_mime_types(vec![
            "image/png".into(),
            "image/jpeg".into(),
            "image/gif".into(),
        ])
        .build();

    policy.check_file(&file)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // 2. Validate MIME type using magic numbers (security!)
    MimeValidator::validate_against_magic(&file, true)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // 3. Process image
    let processed = ImageProcessor::strip_exif(&file)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 4. Create thumbnail
    let thumbnail = ImageProcessor::create_thumbnail(&processed, 200, 200)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 5. Store original and thumbnail
    let original = storage.store(processed).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let thumb = storage.store(thumbnail).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 6. Return success with file IDs
    Ok((
        StatusCode::OK,
        format!("Uploaded! Original: {}, Thumbnail: {}", original.id, thumb.id),
    ))
}
```

### App Setup

```rust
use acton_htmx::middleware::serve_file;
use axum::{Router, routing::{get, post}};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let storage = Arc::new(
        LocalFileStorage::new(PathBuf::from("/var/app/uploads"))?
    );

    let app = Router::new()
        .route("/upload", post(upload_profile_picture))
        .route("/files/:id", get(serve_file::<LocalFileStorage>))
        .with_state(storage);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

## Next Steps

- Learn about [Form Handling](04-forms.md) for advanced form patterns
- See [Authentication](03-authentication.md) for protecting upload endpoints
- Read [Deployment](05-deployment.md) for production file storage configuration

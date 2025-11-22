# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

acton-htmx is an opinionated Rust web framework for server-rendered HTMX applications. It combines Axum's performance with HTMX's hypermedia-driven architecture, reusing 60-70% of infrastructure from the Acton ecosystem (acton-service and acton-reactive).

**Status**: Phase 1 Complete (Week 12) - Documentation & Examples ✅

## Build and Development Commands

```bash
# Build
cargo build                           # Debug build
cargo build --release                 # Release build
cargo check                           # Quick syntax/type check (prefer over build)

# Test
cargo test                            # Run all tests
cargo test -p acton-htmx              # Test specific crate
cargo test test_name -- --nocapture   # Run single test with output
cargo nextest run                     # Faster test runner (if installed)

# Lint
cargo clippy -- -D warnings           # Run clippy (pedantic+nursery enforced in CI)
cargo fmt                             # Format code
cargo fmt -- --check                  # Check formatting without changes

# Documentation
cargo doc --no-deps --open            # Build and view API docs

# Development server with hot reload
cargo watch -x 'run --example basic_server'

# Database (when implemented)
sqlx database create
sqlx migrate run
sqlx migrate revert
```

## Architecture

### Workspace Structure

```
acton-htmx/                 # Workspace root
├── acton-htmx/             # Main framework crate (library)
├── acton-htmx-cli/         # CLI tool crate (binary)
├── acton-htmx-macros/      # Procedural macros crate
└── .claude/                # Architecture documentation
```

### Key Integration Points

**From acton-service** (located at `/home/rodzilla/projects/acton-service`):
- Configuration system (XDG + figment)
- Observability (OpenTelemetry + tracing)
- Middleware (compression, CORS, rate limiting)
- Connection pools (PostgreSQL via SQLx, Redis)
- Health checks

**From acton-reactive** (located at `/home/rodzilla/projects/acton-reactive`):
- Actor runtime for background jobs
- Session state management via agents
- Flash message coordination
- Real-time features (SSE)

### Core Framework Modules

- `htmx/` - HTMX response types (HxRedirect, HxTrigger, HxSwapOob, etc.)
- `template/` - Askama integration with HTMX helpers and validation error rendering
- `auth/` - Session-based authentication with Argon2 password hashing
- `extractors/` - Axum extractors for sessions, flash messages, HTMX requests, validated forms
- `middleware/` - CSRF protection, session handling, security headers
- `agents/` - acton-reactive agents for sessions, CSRF, flash messages
- `state/` - AppState combining both ecosystems
- `config/` - XDG-compliant configuration
- `forms/` - Declarative form builders with HTMX integration

## Code Quality Standards

### Enforced via Workspace Cargo.toml

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
warnings = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
cargo = { level = "warn", priority = -1 }
```

### Requirements

- Zero `unsafe` code (enforced via `#![forbid(unsafe_code)]`)
- No clippy warnings (pedantic + nursery)
- All public items must be documented
- Conventional Commits for all commit messages

### Dependency Management

Always use `cargo add` or `cargo remove` to manage dependencies so the latest versions are selected.

## Commit Convention

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

Types: feat, fix, docs, style, refactor, perf, test, chore
Scopes: htmx, template, auth, csrf, middleware, config, cli, macros
```

## Expert Resources

Use the `@agent-acton-reactive-expert` agent for guidance on acton-reactive crate patterns, best practices, and API usage.

## Key Dependencies

- `axum = "0.8"` - Web framework
- `acton-reactive = "5"` - Actor runtime
- `askama = "0.14"` - Template engine (compile-time)
- `axum-htmx = "0.8"` - HTMX header parsing
- `validator = "0.20"` - Form validation
- `argon2 = "0.5"` - Password hashing
- `sqlx = "0.8"` - Database (PostgreSQL, SQLite)

## Design Principles

1. **Convention Over Configuration** - Smart defaults, no decision paralysis
2. **Security by Default** - CSRF, secure sessions, security headers enabled
3. **HTMX-First** - Every API optimized for hypermedia-driven applications
4. **Type Safety** - Compile-time guarantees via Rust's type system
5. **Idiomatic Excellence** - Generated code exemplifies Rust best practices

## Implementation Status

### Completed Features (Weeks 1-10)

**Week 1-2: Foundation**
- ✅ Workspace structure with strict lints (forbid unsafe, deny warnings)
- ✅ Configuration system (XDG + figment integration)
- ✅ Observability (OpenTelemetry + tracing)
- ✅ acton-reactive runtime integration

**Week 3-4: HTMX Layer**
- ✅ axum-htmx integration (extractors and responders)
- ✅ HxSwapOob for out-of-band swaps
- ✅ HxTemplate trait for automatic partial rendering

**Week 5-6: Templates & Forms**
- ✅ Askama integration with HTMX helpers
- ✅ Template helper functions (hx_post, hx_get, etc.)
- ✅ Declarative form builders (FormBuilder, FieldBuilder)
- ✅ Form validation integration

**Week 7-8: Sessions & Auth**
- ✅ SessionManagerAgent (acton-reactive based)
- ✅ SessionMiddleware with cookie handling
- ✅ Flash message system (FlashCoordinatorAgent)
- ✅ Password hashing (Argon2id)
- ✅ Auth extractors (Authenticated, OptionalAuth)

**Week 9: CSRF Protection**
- ✅ CsrfManagerAgent with token generation and validation
- ✅ CsrfMiddleware for automatic protection
- ✅ CsrfTokenExtractor for handlers
- ✅ Template helper: csrf_token_with()

**Week 10: Security & Validation**
- ✅ SecurityHeadersMiddleware (X-Frame-Options, CSP, HSTS, etc.)
- ✅ ValidatedForm extractor with validator crate integration
- ✅ Validation error rendering helpers
- ✅ Template helpers: validation_errors_for(), has_error(), error_class()

**Week 11: CLI Implementation**
- ✅ `acton-htmx new <name>` - Project scaffolding with full template structure
- ✅ `acton-htmx dev` - Development server with optional cargo-watch hot reload
- ✅ `acton-htmx db migrate` - Run SQLx migrations
- ✅ `acton-htmx db reset` - Drop, recreate, and migrate database
- ✅ `acton-htmx db create <name>` - Create new migration file
- ✅ Complete project templates (Cargo.toml, main.rs, handlers, models, templates, migrations)
- ✅ Handlebars-based template rendering with project-specific context
- ✅ Beautiful CLI output with console styling (colors, spinners, progress)

### CLI Commands

```bash
# Create new project
acton-htmx new my-app

# Start development server
cd my-app
acton-htmx dev

# Database management
acton-htmx db migrate              # Run migrations
acton-htmx db reset                # Reset database
acton-htmx db create add_users     # Create migration
```

### Test Coverage

- **12 tests passing** (CLI + unit + integration tests)
- **58 doctests passing** (all documentation examples)
- Zero test failures
- Comprehensive coverage of:
  - CLI: Project scaffolding, template generation, crate name validation
  - Agent behavior (CSRF, sessions, flash messages)
  - Middleware functionality (auth, CSRF, security headers, sessions)
  - Extractors (sessions, CSRF, validated forms)
  - Template helpers (HTMX attributes, validation errors)
  - Form builders and validation

## Documentation References

### User Guides (docs/guides/)
- `00-getting-started.md` - First acton-htmx application
- `01-htmx-responses.md` - Complete HTMX response type guide
- `02-templates.md` - Askama integration and patterns
- `03-authentication.md` - Sessions, passwords, security
- `04-forms.md` - Validation and HTMX patterns
- `05-deployment.md` - Production deployment guide
- `06-file-uploads.md` - File upload, validation, processing, and serving

### Examples (docs/examples/)
- `blog-crud.md` - Complete blog with CRUD operations

### Architecture (.claude/)
- `architecture-overview.md` - System architecture
- `development-workflow.md` - Development environment setup
- `crate-structure.md` - Workspace organization
- `technical-decisions.md` - Architecture decision log
- `phase-1-implementation-plan.md` - Implementation roadmap
- `acton-htmx-vision.md` - Project vision and goals

## Phase 1 Completion (Week 12)

All Week 12 deliverables completed:

**Documentation Delivered**:
- ✅ 6 comprehensive user guides (Getting Started, HTMX, Templates, Auth, Forms, Deployment)
- ✅ Complete blog CRUD example with code samples
- ✅ Updated README with feature showcase
- ✅ API documentation via rustdoc
- ✅ All public APIs documented with examples

**Guides Cover**:
- Getting started (installation, first app, basic concepts)
- HTMX response types (all 10+ response helpers with examples)
- Template integration (Askama, partials, layouts, HTMX patterns)
- Authentication & security (sessions, passwords, CSRF, security headers)
- Form handling (validation, HTMX patterns, error display)
- Deployment (Docker, systemd, Kubernetes, monitoring, performance)

**Example Application**:
- Complete blog with CRUD operations
- Authentication and authorization
- HTMX inline editing
- Flash messages
- Validation and error handling

**Success Criteria Met**:
- ✅ < 30 minutes to create and deploy CRUD app with auth (via CLI)
- ✅ Zero clippy warnings (pedantic + nursery)
- ✅ Zero unsafe code (enforced)
- ✅ Documentation complete and comprehensive
- ✅ Generated code is idiomatic and exemplary

## Phase 2 Progress (Weeks 7-9)

**Week 7: File Upload Foundation** ✅
- ✅ FileStorage trait - Backend-agnostic file operations
- ✅ LocalFileStorage - UUID-based filesystem implementation
- ✅ UploadedFile - Type-safe upload handling with validation
- ✅ StoredFile - Persisted file metadata with serialization
- ✅ FileUpload extractor - Single file uploads with streaming
- ✅ MultiFileUpload extractor - Multiple file uploads
- ✅ 18 passing tests (14 storage + 4 extractors)

**Week 8: Security & Processing** ✅
- ✅ MimeValidator - Magic number detection (using `infer` crate)
- ✅ ImageProcessor - Thumbnails, resizing, format conversion, EXIF stripping
- ✅ VirusScanner trait - ClamAV integration framework (NoOpScanner, ClamAvScanner placeholder, QuarantineScanner)
- ✅ UploadPolicy - Role-based upload restrictions, MIME type filtering, quota enforcement
- ✅ 52 passing storage tests (all modules)
- ✅ Zero clippy lints (pedantic + nursery + all targets)
- ✅ Zero unsafe code

**Week 9: Upload UI & File Serving** ✅
- ✅ File upload form helpers (FileFieldBuilder)
  - Accept MIME types and file extensions
  - Multiple file selection
  - Client-side size hints
  - Preview and drag-drop attributes
  - Progress endpoint integration
- ✅ File serving middleware
  - Range request support (streaming, resumable downloads)
  - Cache headers (ETag, Last-Modified, Cache-Control)
  - Conditional requests (If-None-Match, If-Range)
  - Access control framework
  - CDN integration hints
- ✅ Comprehensive testing
  - 7 new form upload tests
  - File serving middleware tests
  - Range request parsing tests
  - Zero clippy lints maintained
- ✅ Complete documentation
  - 06-file-uploads.md guide with security best practices
  - Form builder examples
  - File serving configuration
  - Complete upload workflow examples

**Quality Metrics**:
- **270+ tests passing** (all modules including new upload features)
- **Zero clippy lints** with `--all-targets` (pedantic + nursery)
- **Production-ready** file serving with HTTP/1.1 compliance
- **Security-first** design (magic number validation, safe defaults)

**Key Features Delivered**:
- Declarative file upload forms with HTMX integration
- HTTP range requests for streaming and resumable downloads
- Proper caching with ETag and Last-Modified headers
- Automatic multipart/form-data encoding
- File preview and drag-drop support hooks
- Upload progress tracking endpoint integration

**Notes**:
- SSE progress tracking and UI templates deferred (can be implemented with existing hooks)
- File serving middleware fully functional with range requests and caching
- Form helpers production-ready with comprehensive attribute support

**Week 10: Email System** ✅
- ✅ Email abstraction (`EmailSender` trait)
  - Clean async trait for sending emails
  - Batch sending support with default implementation
  - Backend-agnostic design
- ✅ SMTP backend (using `lettre` crate)
  - Full SMTP support with STARTTLS
  - Environment variable configuration
  - HTML + plain text multipart emails
  - CC, BCC, Reply-To support
  - Comprehensive error handling
- ✅ AWS SES backend
  - AWS SDK v2 integration (feature-gated)
  - Default credential provider chain
  - HTML + plain text support
  - Graceful fallback when feature disabled
- ✅ Console backend (development mode)
  - Beautiful console output for development
  - Verbose mode with full email content logging
  - No external dependencies required
- ✅ Email template system (Askama integration)
  - `EmailTemplate` trait for rendering HTML + text
  - `SimpleEmailTemplate` helper trait
  - Compile-time template validation
  - Template inheritance support
- ✅ Common email templates
  - Welcome email (HTML + text)
  - Email verification (HTML + text with code)
  - Password reset (HTML + text with expiry)
  - Password changed notification (HTML + text)
  - Account deletion confirmation (HTML + text)
  - Professional styling with inline CSS
- ✅ Background job integration
  - `SendEmailJob` for async email sending
  - Job serialization/deserialization
  - Configurable retry logic (3 retries by default)
  - 30-second timeout per email
- ✅ Email testing utilities
  - `MockEmailSender` for testing
  - Email assertion helpers
  - Send count tracking
  - Subject and recipient verification
  - Comprehensive test coverage

**Quality Metrics**:
- **300+ tests passing** (all modules including new email system)
- **Zero clippy lints** with `--all-targets` (pedantic + nursery)
- **Production-ready** email sending with multiple backends
- **Security-first** design (no credentials in code, env vars only)

**Key Features Delivered**:
- Multiple email backends (SMTP, AWS SES, Console)
- Template system with Askama integration
- Common authentication email flows
- Background job integration
- Comprehensive testing utilities
- Beautiful development mode output

**Dependencies Added**:
- `lettre = "0.11.19"` - SMTP email sending (with tokio support)
- `aws-sdk-sesv2 = "1.82.0"` - AWS SES integration (optional feature)
- `aws-config = "1.8.11"` - AWS SDK configuration (optional feature)

**Example Usage**:
```rust
// SMTP backend
let smtp = SmtpBackend::from_env()?;
let email = Email::new()
    .to("user@example.com")
    .from("noreply@myapp.com")
    .subject("Welcome!")
    .text("Welcome to our app!");
smtp.send(email).await?;

// With templates
#[derive(Template)]
#[template(path = "emails/welcome.html")]
struct WelcomeEmail { name: String }

impl SimpleEmailTemplate for WelcomeEmail {}

let template = WelcomeEmail { name: "Alice".to_string() };
let email = Email::from_template(&template)?
    .to("alice@example.com")
    .from("noreply@myapp.com")
    .subject("Welcome!");
```

**Notes**:
- AWS SES backend is feature-gated (`aws-sdk-sesv2` feature)
- Console backend is perfect for local development (no SMTP server needed)
- All backends share the same `EmailSender` trait for easy swapping
- Email validation happens before sending (comprehensive checks)
- Custom headers support deferred (requires lettre-specific header types)

# acton-htmx Documentation

Welcome to the acton-htmx documentation! This guide will help you build production-ready HTMX applications in Rust.

## Quick Links

- **[Getting Started →](guides/00-getting-started.md)** - Your first acton-htmx application
- **[Complete Example →](examples/blog-crud.md)** - Blog with full CRUD operations
- **[API Documentation →](../target/doc/acton_htmx/index.html)** - Generated API docs (run `cargo doc --open`)

## Learning Path

### For Beginners

Start here if you're new to acton-htmx:

1. **[Getting Started](guides/00-getting-started.md)** - Install CLI, create first project
2. **[HTMX Responses](guides/01-htmx-responses.md)** - Learn HTMX response types
3. **[Templates](guides/02-templates.md)** - Build views with Askama
4. **[Blog Example](examples/blog-crud.md)** - See it all working together

### For Building Features

Reference these guides when implementing specific features:

- **[Authentication](guides/03-authentication.md)** - Add login, registration, sessions
- **[Forms](guides/04-forms.md)** - Handle and validate forms
- **[HTMX Patterns](guides/01-htmx-responses.md#htmx-patterns)** - Common HTMX use cases

### For Production

Ready to deploy? Start here:

- **[Deployment Guide](guides/05-deployment.md)** - Deploy to production
- **[Security Checklist](guides/03-authentication.md#best-practices)** - Security best practices
- **[Performance Tuning](guides/05-deployment.md#performance-tuning)** - Optimize for production

## User Guides

### Core Concepts

| Guide | Topics | When to Read |
|-------|--------|--------------|
| **[Getting Started](guides/00-getting-started.md)** | Installation, first app, basic concepts | Start here |
| **[HTMX Responses](guides/01-htmx-responses.md)** | All response types, patterns, best practices | Building dynamic UIs |
| **[Templates](guides/02-templates.md)** | Askama, layouts, partials, HTMX integration | Creating views |

### Features

| Guide | Topics | When to Read |
|-------|--------|--------------|
| **[Authentication](guides/03-authentication.md)** | Sessions, passwords, CSRF, security | Adding user accounts |
| **[Forms](guides/04-forms.md)** | Validation, error display, HTMX patterns | Building forms |
| **[Deployment](guides/05-deployment.md)** | Docker, systemd, K8s, monitoring | Going to production |

## Examples

### Complete Applications

- **[Blog with CRUD](examples/blog-crud.md)** - Full-featured blog
  - List, create, edit, delete posts
  - Authentication and authorization
  - HTMX inline editing
  - Flash messages
  - Validation

### Code Snippets

Each guide contains numerous code examples:

- **Getting Started**: First handler, database queries, templates
- **HTMX Responses**: All 10+ response types with examples
- **Templates**: Layouts, partials, forms, HTMX patterns
- **Authentication**: Login, register, logout, protected routes
- **Forms**: Validation, error display, HTMX integration
- **Deployment**: Docker, Nginx, systemd, monitoring

## Architecture Documentation

For deeper understanding of the framework:

- **[Vision](../acton-htmx-vision.md)** - Project goals and philosophy
- **[Architecture Overview](../.claude/architecture-overview.md)** - System design
- **[Implementation Plan](../.claude/phase-1-implementation-plan.md)** - Development roadmap
- **[Technical Decisions](../.claude/technical-decisions.md)** - ADR log
- **[Development Workflow](../.claude/development-workflow.md)** - Contributing guide

## API Documentation

Generate and view the complete API documentation:

```bash
cargo doc --no-deps --open
```

This will open the full rustdoc documentation in your browser, including:
- All public modules, structs, and functions
- Code examples for each API
- Module-level documentation
- Type signatures and trait implementations

## Quick Reference

### Common Commands

```bash
# Project creation
acton-htmx new myapp

# Development
acton-htmx dev                    # Start dev server
cargo check                       # Quick type check
cargo test                        # Run tests

# Database
acton-htmx db migrate             # Run migrations
acton-htmx db reset               # Reset database
acton-htmx db create <name>       # Create migration

# Building
cargo build --release             # Production build
cargo clippy -- -D warnings       # Lint code
```

### Key Concepts

| Concept | Description | Learn More |
|---------|-------------|------------|
| **HTMX Responses** | Type-safe headers for HTMX | [Guide](guides/01-htmx-responses.md) |
| **HxSwapOob** | Update multiple elements | [Guide](guides/01-htmx-responses.md#10-hxswapoob---out-of-band-swaps) |
| **HxTemplate** | Auto partial rendering | [Guide](guides/02-templates.md#automatic-partial-rendering) |
| **Sessions** | Actor-based session management | [Guide](guides/03-authentication.md#session-management) |
| **CSRF** | Automatic token protection | [Guide](guides/03-authentication.md#csrf-protection) |
| **Flash Messages** | One-time user messages | [Guide](guides/03-authentication.md#flash-messages) |

### HTMX Response Types

Quick reference for HTMX responses:

| Response | Purpose | Example |
|----------|---------|---------|
| `HxRedirect` | Client-side redirect | Navigation after form |
| `HxRefresh` | Reload current page | After delete |
| `HxTrigger` | Fire JS events | Notify components |
| `HxSwapOob` | Update multiple elements | Flash + content |
| `HxPushUrl` | Update URL bar | SPA-like navigation |

See [complete guide](guides/01-htmx-responses.md) for all response types.

## Getting Help

### Documentation Issues

If you find errors or gaps in the documentation:
1. Check the [examples](examples/) for working code
2. Search [GitHub issues](https://github.com/yourusername/acton-htmx/issues)
3. Open a new issue with the "documentation" label

### Feature Questions

For questions about using specific features:
1. Check the relevant guide
2. Look at the [blog example](examples/blog-crud.md)
3. Ask in [GitHub Discussions](https://github.com/yourusername/acton-htmx/discussions)

### Contributing

We welcome documentation contributions!
- Fix typos or improve clarity
- Add more examples
- Translate guides
- Create new tutorials

See [Development Workflow](../.claude/development-workflow.md) for contribution guidelines.

## What's Next?

### Start Building

1. **[Install and create your first app →](guides/00-getting-started.md)**
2. **[Learn HTMX patterns →](guides/01-htmx-responses.md)**
3. **[Build a complete example →](examples/blog-crud.md)**

### Learn More

- Join our [community discussions](https://github.com/yourusername/acton-htmx/discussions)
- Read the [vision document](../acton-htmx-vision.md)
- Explore the [architecture](../.claude/architecture-overview.md)

### Stay Updated

- Watch the [GitHub repository](https://github.com/yourusername/acton-htmx)
- Follow [release notes](https://github.com/yourusername/acton-htmx/releases)
- Check the [roadmap](../.claude/phase-1-implementation-plan.md) for upcoming features

---

**Ready to start?** [Install acton-htmx and create your first app →](guides/00-getting-started.md)

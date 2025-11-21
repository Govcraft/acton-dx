# Phase 2 Implementation Plan

**Status**: Phase 1 Complete (v1.0.0-alpha) → Planning Phase 2 (v1.1.0)
**Timeline**: 16 weeks
**Target Release**: v1.1.0-beta

---

## Quick Summary

Phase 2 transforms acton-htmx from a solid alpha framework into a production powerhouse through intelligent scaffolding and automation.

**Key Features:**
1. CRUD scaffold generator (Weeks 1-3)
2. Background job system (Weeks 4-6)
3. File upload system (Weeks 7-9)
4. Email system (Week 10)
5. OAuth2 integration (Weeks 11-12)
6. Authorization policies (Weeks 13-14)
7. Deployment tools (Weeks 15-16)

**Impact**: 10 hours → 1 hour to build production app

---

## Detailed Planning Documents

Complete implementation details are available in:
- `.claude/phase-2-implementation-plan.md` - Week-by-week breakdown with tasks
- `.claude/phase-2-overview.md` - Executive summary and comparisons
- `.claude/implementation-progress.md` - Progress tracking (will be updated weekly)

---

## Phase 1 Achievements (v1.0.0-alpha)

✅ **Core Framework** (257 tests, 0 warnings)
- HTMX integration (10+ response types)
- Askama template system
- Form builders with validation
- Session-based authentication
- CSRF protection
- Security headers
- CLI tools (new, dev, db commands)
- Comprehensive documentation (6 guides)

---

## Phase 2 Goals

### 1. CRUD Scaffolding (Weeks 1-3)

**Command:**
```bash
acton-htmx scaffold crud Post title:string content:text author:references:User
```

**Generates:**
- SeaORM model with validation
- Migration with constraints
- Form structs (CreateForm, UpdateForm)
- HTMX handlers (list, show, new, edit, delete)
- Askama templates
- Integration tests
- Route registration

**Impact**: 60 min → 1 min to add CRUD resource

---

### 2. Background Jobs (Weeks 4-6)

**Features:**
- Redis-backed job queue
- Type-safe job definitions
- Retry logic with exponential backoff
- Dead letter queue
- Cron scheduling
- Job monitoring dashboard
- Graceful shutdown

**Example:**
```rust
#[async_trait]
impl Job for WelcomeEmailJob {
    async fn execute(&self, ctx: &JobContext) -> Result<()> {
        ctx.email.send_welcome(self.user_id).await
    }
}

// Enqueue
state.jobs.enqueue(WelcomeEmailJob { user_id }).await?;
```

---

### 3. File Uploads (Weeks 7-9)

**Features:**
- Multipart form parsing with streaming
- MIME validation (magic numbers)
- Virus scanning (ClamAV)
- Storage backends (local, S3, Azure)
- Image processing (thumbnails)
- Upload progress with SSE
- Drag-drop UI

**Example:**
```rust
async fn create_post(
    FileUpload(image): FileUpload,
) -> Result<Response> {
    image.validate_mime(&["image/png", "image/jpeg"])?;
    let stored = state.storage.store(image).await?;
}
```

---

### 4. Email System (Week 10)

**Features:**
- Email backends (SMTP, AWS SES, SendGrid)
- Askama templates (HTML + text)
- Common flows (welcome, verification, reset)
- Background job integration
- Delivery tracking

**Example:**
```rust
state.email.send(
    Email::new()
        .to(&user.email)
        .template("welcome")
        .data(json!({ "name": user.name }))
).await?;
```

---

### 5. OAuth2 (Weeks 11-12)

**Providers:**
- Google OAuth2
- GitHub OAuth2
- Generic OIDC

**Command:**
```bash
acton-htmx scaffold oauth2 google
```

**Features:**
- Account linking
- Security (state, nonce, PKCE)
- Beautiful login UI

---

### 6. Authorization (Weeks 13-14)

**Features:**
- Declarative policy system
- Action-based checks (view, create, update, delete)
- Role-based access control
- Template helpers

**Example:**
```rust
#[derive(Policy)]
pub struct PostPolicy;

impl PolicyRules<Post> for PostPolicy {
    fn can_update(user: &User, post: &Post) -> bool {
        user.id == post.author_id || user.is_admin()
    }
}

// In handler
async fn update_post(
    Authorized(user, post): Authorized<User, Post, Update>,
) -> Result<Response> {
    // Guaranteed authorized
}
```

---

### 7. Deployment (Weeks 15-16)

**Command:**
```bash
acton-htmx generate deployment docker
```

**Generates:**
- Optimized Dockerfile (< 50MB)
- docker-compose.yml
- Kubernetes manifests (optional)
- Systemd service files
- Monitoring setup (Prometheus, Grafana)
- Production checklist

---

## Timeline

```
Week 1-3:   CRUD Scaffolding
Week 4-6:   Background Jobs
Week 7-9:   File Uploads
Week 10:    Email System
Week 11-12: OAuth2
Week 13-14: Authorization
Week 15-16: Deployment

Total: 16 weeks to v1.1.0
```

### Beta Releases

- **v1.1.0-beta.1** (Week 6) - CRUD + Jobs + Uploads
- **v1.1.0-beta.2** (Week 12) - Email + OAuth2
- **v1.1.0-beta.3** (Week 16) - Authorization + Deployment

---

## Success Criteria

### Technical
- [ ] 95%+ test coverage
- [ ] Zero critical CVEs
- [ ] < 50MB Docker image
- [ ] < 15 min to scaffold CRUD
- [ ] All scaffolds pass clippy pedantic

### Community
- [ ] 2,000+ GitHub stars
- [ ] 100+ production deployments
- [ ] Active Discord (500+ members)

### Developer Experience
- [ ] 1 hour from zero to production app
- [ ] "Easiest Rust web framework" feedback

---

## Comparison: Phase 1 vs Phase 2

| Task | v1.0.0-alpha | v1.1.0 |
|------|--------------|--------|
| Add CRUD | 60 min | 1 min |
| Add background job | 30 min | 5 min |
| Add file upload | 90 min | 10 min |
| Add email | 60 min | 5 min |
| Add OAuth2 | 120 min | 5 min |
| Add authorization | 45 min | 10 min |
| Deploy | 180 min | 10 min |
| **Total** | **~10 hours** | **~1 hour** |

**10x productivity improvement**

---

## Quality Standards

All scaffolded code must:
- ✅ Pass clippy pedantic + nursery (zero warnings)
- ✅ Be fully documented (rustdoc)
- ✅ Include tests (95%+ coverage)
- ✅ Follow Rust API guidelines
- ✅ Use idiomatic patterns
- ✅ Have zero unsafe code

---

## Getting Started

**For Contributors:**
1. Review `.claude/phase-2-implementation-plan.md`
2. Start with Week 1 tasks
3. Follow quality standards
4. Submit PRs with conventional commits

**For Users:**
1. Use v1.0.0-alpha for current projects
2. Watch for v1.1.0-beta releases
3. Provide feedback
4. Upgrade when ready

---

## Phase 3 Preview

After v1.1.0:
- Real-time features (WebSocket, SSE)
- Multi-tenancy support
- Admin panel generator
- Internationalization (i18n)
- Community plugin system

---

**Next Step**: Begin Week 1 (CRUD Scaffold Architecture) on 2025-11-22

For detailed information, see `.claude/phase-2-implementation-plan.md` (91KB, 16-week breakdown)

//! Deployment templates for Docker and containerization

/// Optimized multi-stage Dockerfile for production builds
///
/// Features:
/// - Multi-stage build (builder + runtime)
/// - Minimal distroless base image
/// - Layer caching optimization
/// - Security best practices
pub const DOCKERFILE: &str = r#"# =============================================================================
# Build Stage
# =============================================================================
FROM rust:1.87-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 app

# Set working directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY config ./config

# Build dependencies (cached layer)
# Create dummy src to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src
COPY templates ./templates
COPY migrations ./migrations
COPY static ./static

# Build application
RUN cargo build --release

# =============================================================================
# Runtime Stage
# =============================================================================
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy binary from builder
COPY --from=builder /app/target/release/{{project_name}} /usr/local/bin/app

# Copy configuration
COPY --from=builder /app/config /app/config

# Copy templates
COPY --from=builder /app/templates /app/templates

# Copy static assets
COPY --from=builder /app/static /app/static

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

# Set working directory
WORKDIR /app

# Run as non-root user (distroless default is nonroot:65532)
USER nonroot:nonroot

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/app", "health"] || exit 1

# Run the application
ENTRYPOINT ["/usr/local/bin/app"]
"#;

/// Docker Compose configuration with all services
///
/// Services:
/// - web: acton-htmx application
/// - db: `PostgreSQL` database
/// - redis: Redis cache/session store
/// - nginx: Reverse proxy (optional)
pub const DOCKER_COMPOSE: &str = r#"version: '3.8'

services:
  # =============================================================================
  # Web Application
  # =============================================================================
  web:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: {{project_name}}_web
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - DATABASE_URL=postgres://app:password@db:5432/{{project_name_snake}}
      - REDIS_URL=redis://redis:6379
      - SERVER_HOST=0.0.0.0
      - SERVER_PORT=8080
    env_file:
      - .env.production
    depends_on:
      db:
        condition: service_healthy
      redis:
        condition: service_healthy
    networks:
      - app-network
    volumes:
      - uploads:/app/uploads
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s

  # =============================================================================
  # PostgreSQL Database
  # =============================================================================
  db:
    image: postgres:16-alpine
    container_name: {{project_name}}_db
    restart: unless-stopped
    environment:
      - POSTGRES_USER=app
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB={{project_name_snake}}
    ports:
      - "5432:5432"
    networks:
      - app-network
    volumes:
      - postgres-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U app"]
      interval: 10s
      timeout: 5s
      retries: 5

  # =============================================================================
  # Redis Cache & Session Store
  # =============================================================================
  redis:
    image: redis:7-alpine
    container_name: {{project_name}}_redis
    restart: unless-stopped
    command: redis-server --appendonly yes
    ports:
      - "6379:6379"
    networks:
      - app-network
    volumes:
      - redis-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

  # =============================================================================
  # Nginx Reverse Proxy (Optional)
  # =============================================================================
  nginx:
    image: nginx:alpine
    container_name: {{project_name}}_nginx
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    networks:
      - app-network
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./static:/var/www/static:ro
      - ./ssl:/etc/nginx/ssl:ro
    depends_on:
      - web
    healthcheck:
      test: ["CMD", "nginx", "-t"]
      interval: 30s
      timeout: 3s
      retries: 3

networks:
  app-network:
    driver: bridge

volumes:
  postgres-data:
  redis-data:
  uploads:
"#;

/// Environment variable template for production
pub const ENV_PRODUCTION: &str = r"# =============================================================================
# Server Configuration
# =============================================================================
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
RUST_LOG=info

# =============================================================================
# Database Configuration
# =============================================================================
DATABASE_URL=postgres://app:CHANGE_ME_PASSWORD@db:5432/{{project_name_snake}}
DATABASE_MAX_CONNECTIONS=10
DATABASE_MIN_CONNECTIONS=2

# =============================================================================
# Redis Configuration
# =============================================================================
REDIS_URL=redis://redis:6379
REDIS_MAX_CONNECTIONS=10

# =============================================================================
# Session Configuration
# =============================================================================
SESSION_SECRET=CHANGE_ME_TO_RANDOM_64_CHAR_STRING
SESSION_TIMEOUT_SECS=86400
SESSION_COOKIE_SECURE=true
SESSION_COOKIE_HTTP_ONLY=true

# =============================================================================
# CSRF Configuration
# =============================================================================
CSRF_ENABLED=true

# =============================================================================
# Security Headers
# =============================================================================
SECURITY_HEADERS_ENABLED=true
HSTS_MAX_AGE=31536000

# =============================================================================
# Cedar Authorization (if enabled)
# =============================================================================
CEDAR_ENABLED=true
CEDAR_POLICY_PATH=policies/app.cedar
CEDAR_CACHE_ENABLED=true
CEDAR_FAIL_OPEN=false

# =============================================================================
# Email Configuration (choose one backend)
# =============================================================================
# SMTP
EMAIL_BACKEND=smtp
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=noreply@example.com
SMTP_PASSWORD=CHANGE_ME
SMTP_FROM=noreply@example.com

# AWS SES (alternative)
# EMAIL_BACKEND=ses
# AWS_REGION=us-east-1
# AWS_ACCESS_KEY_ID=CHANGE_ME
# AWS_SECRET_ACCESS_KEY=CHANGE_ME
# SES_FROM=noreply@example.com

# =============================================================================
# File Upload Configuration
# =============================================================================
UPLOAD_MAX_SIZE_MB=10
UPLOAD_STORAGE_BACKEND=local
UPLOAD_LOCAL_PATH=/app/uploads

# S3 Storage (alternative)
# UPLOAD_STORAGE_BACKEND=s3
# S3_BUCKET=my-uploads
# S3_REGION=us-east-1
# S3_ACCESS_KEY_ID=CHANGE_ME
# S3_SECRET_ACCESS_KEY=CHANGE_ME

# =============================================================================
# Background Jobs
# =============================================================================
JOBS_MAX_WORKERS=4
JOBS_POLL_INTERVAL_MS=1000

# =============================================================================
# OAuth2 Providers (if enabled)
# =============================================================================
# Google
OAUTH2_GOOGLE_CLIENT_ID=CHANGE_ME
OAUTH2_GOOGLE_CLIENT_SECRET=CHANGE_ME
OAUTH2_GOOGLE_REDIRECT_URI=https://yourdomain.com/auth/google/callback

# GitHub
OAUTH2_GITHUB_CLIENT_ID=CHANGE_ME
OAUTH2_GITHUB_CLIENT_SECRET=CHANGE_ME
OAUTH2_GITHUB_REDIRECT_URI=https://yourdomain.com/auth/github/callback
";

/// Nginx reverse proxy configuration
pub const NGINX_CONF: &str = r#"user nginx;
worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for"';

    access_log /var/log/nginx/access.log main;

    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    types_hash_max_size 2048;

    # Gzip compression
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_types text/plain text/css text/xml text/javascript
               application/x-javascript application/xml+rss
               application/json application/javascript;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=general:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=auth:10m rate=5r/m;

    # Upstream application server
    upstream app_server {
        server web:8080;
        keepalive 32;
    }

    # Redirect HTTP to HTTPS
    server {
        listen 80;
        server_name _;
        return 301 https://$host$request_uri;
    }

    # HTTPS server
    server {
        listen 443 ssl http2;
        server_name _;

        # SSL configuration
        ssl_certificate /etc/nginx/ssl/cert.pem;
        ssl_certificate_key /etc/nginx/ssl/key.pem;
        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_ciphers HIGH:!aNULL:!MD5;
        ssl_prefer_server_ciphers on;

        # Security headers
        add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
        add_header X-Frame-Options "DENY" always;
        add_header X-Content-Type-Options "nosniff" always;
        add_header X-XSS-Protection "1; mode=block" always;

        # Static files
        location /static/ {
            alias /var/www/static/;
            expires 1y;
            add_header Cache-Control "public, immutable";
        }

        # Rate limiting for auth endpoints
        location ~ ^/auth/(login|register) {
            limit_req zone=auth burst=5 nodelay;
            proxy_pass http://app_server;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }

        # Application proxy
        location / {
            limit_req zone=general burst=20 nodelay;
            proxy_pass http://app_server;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
            proxy_http_version 1.1;
            proxy_set_header Connection "";
        }

        # Health check endpoint (no rate limiting)
        location /health {
            proxy_pass http://app_server;
            access_log off;
        }
    }
}
"#;

/// Docker ignore file for optimal builds
pub const DOCKERIGNORE: &str = r"# Build artifacts
target/
Cargo.lock

# IDE
.vscode/
.idea/
*.swp
*.swo
*~

# Git
.git/
.gitignore

# Docker
Dockerfile
docker-compose.yml
.dockerignore

# Documentation
README.md
docs/
.claude/

# CI/CD
.github/
.gitlab-ci.yml

# Environment
.env
.env.*
!.env.example

# Logs
*.log

# OS
.DS_Store
Thumbs.db

# Test coverage
coverage/
*.profraw
*.profdata

# Temporary files
tmp/
temp/
";

/// Production deployment README
pub const DEPLOYMENT_README: &str = r"# Production Deployment Guide

This directory contains all the necessary files for deploying {{project_name}} to production using Docker.

## 📦 Files Generated

- `Dockerfile` - Optimized multi-stage build for production
- `docker-compose.yml` - Complete stack (web, database, redis, nginx)
- `.env.production` - Environment variables template
- `nginx.conf` - Reverse proxy configuration
- `.dockerignore` - Build optimization

## 🚀 Quick Start

### 1. Configure Environment Variables

```bash
# Copy and edit environment variables
cp .env.production .env

# IMPORTANT: Change these values:
# - SESSION_SECRET (generate with: openssl rand -hex 32)
# - Database passwords
# - Email credentials
# - OAuth2 credentials (if using)
```

### 2. Generate SSL Certificates

```bash
# Create SSL directory
mkdir -p ssl

# Option A: Self-signed (development/testing)
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout ssl/key.pem -out ssl/cert.pem

# Option B: Let's Encrypt (production)
# Use certbot with your domain
```

### 3. Build and Start

```bash
# Build the application
docker-compose build

# Start all services
docker-compose up -d

# Check status
docker-compose ps

# View logs
docker-compose logs -f web
```

### 4. Run Database Migrations

```bash
# Run migrations inside container
docker-compose exec web /usr/local/bin/app migrate

# Or run manually with sqlx-cli
docker-compose exec db psql -U app -d {{project_name_snake}} -f /app/migrations/001_create_users.sql
```

### 5. Verify Deployment

```bash
# Health check
curl http://localhost:8080/health

# Or with HTTPS through Nginx
curl https://localhost/health
```

## 📊 Monitoring

### Health Checks

All services have health checks configured:

```bash
# Check all service health
docker-compose ps

# Web health endpoint
curl http://localhost:8080/health

# Database health
docker-compose exec db pg_isready -U app

# Redis health
docker-compose exec redis redis-cli ping
```

### Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f web
docker-compose logs -f db
docker-compose logs -f redis

# Last 100 lines
docker-compose logs --tail=100 web
```

### Metrics

```bash
# Container stats
docker stats

# Application metrics (if enabled)
curl http://localhost:8080/metrics
```

## 🔧 Maintenance

### Update Application

```bash
# Pull latest code
git pull origin main

# Rebuild and restart
docker-compose build web
docker-compose up -d web

# Run new migrations if needed
docker-compose exec web /usr/local/bin/app migrate
```

### Backup Database

```bash
# Backup PostgreSQL
docker-compose exec db pg_dump -U app {{project_name_snake}} > backup.sql

# Restore
docker-compose exec -T db psql -U app {{project_name_snake}} < backup.sql
```

### Backup Redis

```bash
# Backup Redis (if persistence enabled)
docker-compose exec redis redis-cli BGSAVE
docker cp {{project_name}}_redis:/data/dump.rdb ./redis-backup.rdb
```

### Scale Workers

```bash
# Scale web service to 3 instances
docker-compose up -d --scale web=3

# Note: You'll need a load balancer in front
```

## 🔒 Security Checklist

- [ ] Changed `SESSION_SECRET` to random value
- [ ] Changed all default passwords
- [ ] SSL/TLS certificates configured
- [ ] HTTPS enforced (HTTP redirects to HTTPS)
- [ ] Security headers enabled
- [ ] Rate limiting configured
- [ ] CSRF protection enabled
- [ ] Database not exposed to public internet
- [ ] Redis not exposed to public internet
- [ ] Environment variables secured
- [ ] Backups configured and tested

## 🌐 Production Considerations

### Resource Limits

Edit `docker-compose.yml` to add resource limits:

```yaml
services:
  web:
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.5'
          memory: 256M
```

### Database Connection Pooling

Adjust `DATABASE_MAX_CONNECTIONS` in `.env` based on your traffic:

- Low traffic: 5-10 connections
- Medium traffic: 10-20 connections
- High traffic: 20-50 connections

### Redis Memory

Set Redis max memory in `docker-compose.yml`:

```yaml
redis:
  command: redis-server --appendonly yes --maxmemory 256mb --maxmemory-policy allkeys-lru
```

### Nginx Tuning

For high traffic, adjust `worker_connections` in `nginx.conf`:

```nginx
events {
    worker_connections 4096;  # Increase from 1024
}
```

## 🐛 Troubleshooting

### Application won't start

```bash
# Check logs
docker-compose logs web

# Common issues:
# - Database not ready: wait for db health check
# - Missing migrations: run migrations
# - Bad environment variables: check .env
```

### Database connection errors

```bash
# Verify database is running
docker-compose ps db

# Test connection
docker-compose exec db psql -U app -d {{project_name_snake}}

# Check environment variable
docker-compose exec web env | grep DATABASE_URL
```

### High memory usage

```bash
# Check container stats
docker stats

# Restart services
docker-compose restart web

# Clear Redis cache
docker-compose exec redis redis-cli FLUSHDB
```

### Nginx 502 errors

```bash
# Check upstream
curl http://localhost:8080/health

# Check nginx logs
docker-compose logs nginx

# Test nginx config
docker-compose exec nginx nginx -t
```

## 📚 Additional Resources

- [Docker Documentation](https://docs.docker.com/)
- [PostgreSQL Tuning](https://wiki.postgresql.org/wiki/Tuning_Your_PostgreSQL_Server)
- [Redis Configuration](https://redis.io/topics/config)
- [Nginx Performance](https://www.nginx.com/blog/tuning-nginx/)

## 🆘 Support

For issues specific to {{project_name}}:
- Check application logs: `docker-compose logs web`
- Review configuration: `.env` file
- Verify migrations: `docker-compose exec web /usr/local/bin/app migrate`

For infrastructure issues:
- Docker Compose: `docker-compose --help`
- Container health: `docker inspect <container>`
- Network debugging: `docker network inspect {{project_name}}_app-network`
";

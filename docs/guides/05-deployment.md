# Deployment Guide

This guide covers deploying acton-htmx applications to production environments.

## Pre-Deployment Checklist

### 1. Security

- [ ] HTTPS enabled with valid TLS certificate
- [ ] Session cookies set to `secure = true`
- [ ] Security headers configured (HSTS, CSP, etc.)
- [ ] CSRF protection enabled
- [ ] Database credentials in environment variables (not config files)
- [ ] Rate limiting enabled on authentication endpoints
- [ ] Input validation on all user inputs
- [ ] SQL injection prevention (use parameterized queries)

### 2. Configuration

- [ ] `config/production.toml` created and reviewed
- [ ] Database connection pool sized appropriately
- [ ] Logging level set to `info` or `warn`
- [ ] External services configured (Redis, email, etc.)
- [ ] Asset compilation completed
- [ ] Database migrations tested

### 3. Performance

- [ ] Database indices created for common queries
- [ ] Static assets served with long cache headers
- [ ] Gzip/Brotli compression enabled
- [ ] Connection pooling configured
- [ ] Redis caching enabled for sessions

### 4. Monitoring

- [ ] Error tracking configured (Sentry, etc.)
- [ ] Log aggregation set up
- [ ] Health check endpoint created
- [ ] Metrics collection enabled (Prometheus, etc.)
- [ ] Uptime monitoring configured

## Build for Production

### Release Build

```bash
cargo build --release
```

The optimized binary will be in `target/release/`.

### Size Optimization

```toml
# Cargo.toml
[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Better optimization
strip = true        # Strip symbols
```

### Cross-Compilation

For deploying to different platforms:

```bash
# Install cross-compilation toolchain
rustup target add x86_64-unknown-linux-musl

# Build for Linux (static binary)
cargo build --release --target x86_64-unknown-linux-musl
```

## Production Configuration

### config/production.toml

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 4  # Number of CPU cores

[database]
url = "${DATABASE_URL}"
max_connections = 20
min_connections = 5
connect_timeout = 30
idle_timeout = 600

[session]
cookie_name = "session_id"
max_age_seconds = 604800  # 7 days
secure = true
http_only = true
same_site = "Strict"

[security]
csrf_enabled = true
rate_limit_enabled = true

[observability]
log_level = "info"
otlp_endpoint = "${OTEL_ENDPOINT}"

[redis]
url = "${REDIS_URL}"
pool_size = 10
```

### Environment Variables

Create a `.env` file for production secrets:

```bash
DATABASE_URL=postgresql://user:pass@host:5432/dbname
REDIS_URL=redis://host:6379
SECRET_KEY=your-secret-key-here  # For session encryption
OTEL_ENDPOINT=http://collector:4317
```

**Never commit `.env` to version control!**

## Database Migrations

### Pre-deployment

```bash
# Test migrations on staging database
DATABASE_URL="postgresql://staging-db" sqlx migrate run

# Backup production database
pg_dump production_db > backup-$(date +%Y%m%d).sql

# Run migrations on production
DATABASE_URL="postgresql://production-db" sqlx migrate run
```

### Zero-Downtime Migrations

1. **Additive changes first** - Add new columns/tables
2. **Deploy code** - Code works with old and new schema
3. **Migrate data** - Copy data to new structure
4. **Deploy again** - Code uses new schema only
5. **Remove old schema** - Drop old columns/tables

Example:

```sql
-- Step 1: Add new column (backwards compatible)
ALTER TABLE users ADD COLUMN email_verified BOOLEAN DEFAULT false;

-- Deploy code that can handle both NULL and boolean values

-- Step 2: Migrate data
UPDATE users SET email_verified = true WHERE verified_at IS NOT NULL;

-- Deploy code that uses email_verified only

-- Step 3: Remove old column
ALTER TABLE users DROP COLUMN verified_at;
```

## Deployment Strategies

### 1. Docker Deployment

#### Dockerfile

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/my-app /usr/local/bin/
COPY --from=builder /app/config /app/config
COPY --from=builder /app/static /app/static
COPY --from=builder /app/templates /app/templates

WORKDIR /app

EXPOSE 8080

CMD ["my-app"]
```

#### docker-compose.yml

```yaml
version: '3.8'

services:
  app:
    build: .
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: postgresql://postgres:password@db:5432/myapp
      REDIS_URL: redis://redis:6379
      RUST_LOG: info
    depends_on:
      - db
      - redis

  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: password
      POSTGRES_DB: myapp
    volumes:
      - postgres_data:/var/lib/postgresql/data

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data

volumes:
  postgres_data:
  redis_data:
```

### 2. Systemd Service

```ini
# /etc/systemd/system/myapp.service
[Unit]
Description=My acton-htmx Application
After=network.target postgresql.service

[Service]
Type=simple
User=myapp
WorkingDirectory=/opt/myapp
Environment="RUST_LOG=info"
Environment="DATABASE_URL=postgresql://..."
ExecStart=/opt/myapp/myapp
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

```bash
# Enable and start service
sudo systemctl enable myapp
sudo systemctl start myapp

# View logs
sudo journalctl -u myapp -f
```

### 3. Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: myapp
spec:
  replicas: 3
  selector:
    matchLabels:
      app: myapp
  template:
    metadata:
      labels:
        app: myapp
    spec:
      containers:
      - name: myapp
        image: myapp:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: myapp-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: myapp-secrets
              key: redis-url
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: myapp
spec:
  selector:
    app: myapp
  ports:
  - port: 80
    targetPort: 8080
  type: LoadBalancer
```

## Reverse Proxy

### Nginx

```nginx
upstream myapp {
    server 127.0.0.1:8080;
}

server {
    listen 80;
    server_name example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name example.com;

    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    # Static files
    location /static/ {
        alias /opt/myapp/static/;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # Application
    location / {
        proxy_pass http://myapp;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support (if using SSE)
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### Caddy

```
example.com {
    encode gzip

    handle /static/* {
        root * /opt/myapp
        file_server
    }

    reverse_proxy localhost:8080
}
```

## Monitoring

### Health Check Endpoint

```rust
async fn health() -> impl axum::response::IntoResponse {
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn ready(State(state): State<ActonHtmxState>) -> impl axum::response::IntoResponse {
    // Check database connection
    match sqlx::query("SELECT 1").fetch_one(&state.db_pool).await {
        Ok(_) => Json(json!({"status": "ready"})),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"status": "not ready"}))
        ),
    }
}

let app = Router::new()
    .route("/health", get(health))
    .route("/health/ready", get(ready))
    .with_state(state);
```

### Prometheus Metrics

```toml
# Cargo.toml
[dependencies]
metrics = "0.22"
metrics-exporter-prometheus = "0.13"
```

```rust
use metrics_exporter_prometheus::PrometheusBuilder;

// In main()
PrometheusBuilder::new().install().unwrap();

// Expose metrics endpoint
async fn metrics_handler() -> String {
    metrics_exporter_prometheus::render()
}

let app = Router::new()
    .route("/metrics", get(metrics_handler));
```

## Performance Tuning

### Database Connection Pool

```toml
[database]
max_connections = 20      # 2-3x number of CPU cores
min_connections = 5       # Keep some connections warm
connect_timeout = 30      # Seconds
idle_timeout = 600        # Close idle connections after 10 min
max_lifetime = 1800       # Recycle connections after 30 min
```

### Redis Caching

```rust
// Cache expensive queries
let cached: Option<Vec<Post>> = state.redis
    .get("posts:popular")
    .await?;

if let Some(posts) = cached {
    return Ok(posts);
}

let posts = load_popular_posts(&state.db_pool).await?;

state.redis
    .set_ex("posts:popular", &posts, 300)  // 5 minute TTL
    .await?;

Ok(posts)
```

## Troubleshooting

### High Memory Usage

Check connection pool settings and actor system:

```bash
# Monitor memory
htop

# Check open connections
ss -tn | grep :8080 | wc -l
```

### Slow Database Queries

Enable query logging:

```toml
[database]
log_statements = true
log_slow_statements = true
slow_statement_threshold = 100  # milliseconds
```

### Session Issues

Check Redis connectivity and session configuration:

```bash
# Test Redis connection
redis-cli -h host -p 6379 ping

# Monitor Redis commands
redis-cli monitor
```

## Backup & Recovery

### Automated Backups

```bash
#!/bin/bash
# backup.sh

DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/backups"

# Backup database
pg_dump $DATABASE_URL | gzip > $BACKUP_DIR/db_$DATE.sql.gz

# Backup uploaded files
tar czf $BACKUP_DIR/uploads_$DATE.tar.gz /opt/myapp/uploads

# Backup Redis (if persistent)
redis-cli --rdb $BACKUP_DIR/redis_$DATE.rdb

# Retention: Keep last 30 days
find $BACKUP_DIR -name "*.gz" -mtime +30 -delete
```

### Recovery

```bash
# Restore database
gunzip < db_20250121_120000.sql.gz | psql $DATABASE_URL

# Restore files
tar xzf uploads_20250121_120000.tar.gz -C /
```

## Scaling

### Horizontal Scaling

1. **Stateless application** - Store sessions in Redis
2. **Load balancer** - Distribute traffic (nginx, HAProxy)
3. **Shared database** - All instances use same PostgreSQL
4. **Shared cache** - All instances use same Redis

### Vertical Scaling

1. **Increase workers** - Match CPU core count
2. **Increase memory** - Larger connection pools
3. **Faster storage** - SSD for database

## Next Steps

- **[Examples](../examples/)** - See deployed example applications
- **[Monitoring Best Practices](../guides/monitoring.md)** - (TODO)
- **[Performance Tuning](../guides/performance.md)** - (TODO)

## Reference

- [Axum Deployment](https://github.com/tokio-rs/axum/blob/main/examples/production_deployment.md)
- [PostgreSQL Performance Tuning](https://wiki.postgresql.org/wiki/Performance_Optimization)
- [Redis Best Practices](https://redis.io/docs/management/optimization/)

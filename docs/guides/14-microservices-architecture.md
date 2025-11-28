# Microservices Architecture Guide

This guide covers the microservices architecture in acton-htmx, including service configuration, deployment strategies, and coordination patterns.

## Overview

Acton-HTMX uses a microservices architecture where optional capabilities are provided by separate services communicating via gRPC:

```
                    +-------------------+
                    |   Web Application |
                    |    (acton-dx)     |
                    +---------+---------+
                              |
        +---------------------+---------------------+
        |         |         |         |           |
   +----v---+ +---v----+ +--v---+ +--v----+ +----v----+
   |  Auth  | |  Data  | |Cedar | | Cache | |  File   |
   |Service | |Service | |Svc   | |Service| | Service |
   +--------+ +--------+ +------+ +-------+ +---------+
```

### Services

| Service | Port | Purpose |
|---------|------|---------|
| **auth-service** | 50051 | Sessions, passwords, CSRF, users |
| **data-service** | 50052 | Database queries and transactions |
| **cedar-service** | 50053 | Authorization policy evaluation |
| **cache-service** | 50054 | Redis operations and rate limiting |
| **email-service** | 50055 | Email sending (SMTP, SES) |
| **file-service** | 50056 | File storage and processing |

## Service Configuration

### Environment Variables

Each service can be configured via environment variables:

```bash
# Auth Service
AUTH_SERVICE_PORT=50051
AUTH_SERVICE_SESSION_TTL=3600
AUTH_SERVICE_CSRF_TOKEN_LENGTH=32

# Data Service
DATA_SERVICE_PORT=50052
DATABASE_URL=postgresql://user:pass@localhost/myapp

# Cache Service
CACHE_SERVICE_PORT=50054
REDIS_URL=redis://localhost:6379

# Email Service
EMAIL_SERVICE_PORT=50055
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=user
SMTP_PASSWORD=secret

# File Service
FILE_SERVICE_PORT=50056
STORAGE_PATH=/var/lib/myapp/uploads
```

### Service URLs in Application

Configure service endpoints in your application's config:

```toml
# config/default.toml
[services]
auth_url = "http://127.0.0.1:50051"
data_url = "http://127.0.0.1:50052"
cedar_url = "http://127.0.0.1:50053"
cache_url = "http://127.0.0.1:50054"
email_url = "http://127.0.0.1:50055"
file_url = "http://127.0.0.1:50056"
```

## CLI Commands

### Starting Services

Use the `acton-dx htmx services` command to manage services:

```bash
# Start all services
acton-dx htmx services start

# Start specific services
acton-dx htmx services start auth data

# Stop services
acton-dx htmx services stop auth

# Check status
acton-dx htmx services status
```

### Development Mode

The `dev` command shows service status and can auto-start services:

```bash
# Start development server (shows service status)
acton-dx htmx dev

# Start with embedded services (single process)
acton-dx htmx dev --embedded-services
```

## Deployment Modes

### Distributed Mode (Production)

In distributed mode, each service runs as a separate process:

```
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│   Container 1   │  │   Container 2   │  │   Container 3   │
│   Web App       │  │  auth-service   │  │  data-service   │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                    │
         └────────────────────┼────────────────────┘
                              │
                    gRPC over network
```

**Advantages:**
- Independent scaling
- Isolated failure domains
- Independent deployment

**Example docker-compose.yml:**

```yaml
version: '3.8'

services:
  web:
    build:
      context: .
      dockerfile: Dockerfile.web
    ports:
      - "8080:8080"
    environment:
      AUTH_URL: http://auth:50051
      DATA_URL: http://data:50052
    depends_on:
      - auth
      - data

  auth:
    build:
      context: .
      dockerfile: Dockerfile.auth
    environment:
      AUTH_SERVICE_PORT: 50051

  data:
    build:
      context: .
      dockerfile: Dockerfile.data
    environment:
      DATA_SERVICE_PORT: 50052
      DATABASE_URL: postgresql://postgres:password@db/myapp
    depends_on:
      - db

  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: password
      POSTGRES_DB: myapp
```

### Embedded Mode (Development/Simple Deployments)

In embedded mode, all services run within the web application process:

```
┌─────────────────────────────────────────┐
│            Single Process               │
│  ┌─────────┐ ┌──────┐ ┌──────┐ ┌─────┐ │
│  │ Web App │ │ Auth │ │ Data │ │Cache│ │
│  └─────────┘ └──────┘ └──────┘ └─────┘ │
└─────────────────────────────────────────┘
```

**Advantages:**
- Simpler deployment (single binary)
- No network overhead
- Easier debugging

**Enable with:**

```bash
acton-dx htmx dev --embedded-services
```

Or via environment variable:

```bash
ACTON_EMBEDDED_SERVICES=true cargo run
```

## Service Coordination Agents

The framework includes acton-reactive agents for coordinating services:

### ServiceCoordinatorAgent

Tracks service health and manages circuit breakers:

```rust
use acton_dx::htmx::agents::{
    ServiceCoordinatorAgent, ServiceId, ServiceState,
    GetServiceStatus, HealthCheckResult, ServiceAvailable,
};

// Spawn the coordinator
let handle = ServiceCoordinatorAgent::spawn(&mut runtime).await?;

// Report service availability
handle.send(ServiceAvailable::new(ServiceId::Auth)).await;

// Report health check results
handle.send(HealthCheckResult::success(ServiceId::Auth, 10)).await;

// Get status
let (req, rx) = GetServiceStatus::new();
handle.send(req).await;
let status = rx.await?;

for (service_id, (state, circuit)) in &status.services {
    println!("{:?}: {:?} (circuit: {:?})", service_id, state, circuit);
}
```

### Circuit Breaker States

Each service has a circuit breaker with three states:

- **Closed**: Normal operation, requests pass through
- **Open**: Service unhealthy, requests fail fast
- **HalfOpen**: Testing if service recovered

```rust
use acton_dx::htmx::agents::{ServiceCoordinatorConfig, CircuitState};

let config = ServiceCoordinatorConfig::new()
    .with_failure_threshold(5)     // Open after 5 failures
    .with_recovery_timeout(30);    // Try recovery after 30s
```

### RateLimiterAgent

Local rate limiting using token bucket algorithm:

```rust
use acton_dx::htmx::agents::{
    RateLimiterAgent, RateLimiterConfig,
    CheckRateLimit,
};

let config = RateLimiterConfig::new()
    .with_bucket_capacity(100)     // Max 100 tokens
    .with_refill_rate(10.0);       // 10 tokens/second

let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config).await?;

// Check rate limit
let (req, rx) = CheckRateLimit::new("user:123".to_string(), 1);
handle.send(req).await;
let result = rx.await?;

if result.allowed {
    // Process request
} else {
    // Rate limited
}
```

### HotReloadCoordinatorAgent

Coordinates file watching and hot reload:

```rust
use acton_dx::htmx::agents::{
    HotReloadCoordinatorAgent, HotReloadConfig,
    FileChanged, ReloadType, HotReloadSubscribe,
};
use std::time::Duration;

let config = HotReloadConfig::new()
    .with_debounce(ReloadType::Templates, Duration::from_millis(100));

let handle = HotReloadCoordinatorAgent::spawn_with_config(&mut runtime, config).await?;

// Subscribe to reload events
let (sub_req, sub_rx) = HotReloadSubscribe::new();
handle.send(sub_req).await;
let mut subscriber = sub_rx.await?;

// Handle reload events
while let Ok(event) = subscriber.recv().await {
    println!("Reload {:?}: {:?}", event.reload_type, event.paths);
}
```

## Health Checks

### Service Health Endpoint

Each service exposes a health endpoint:

```bash
# Check auth service health
curl http://localhost:50051/health

# Response
{
    "status": "healthy",
    "service": "auth-service",
    "version": "1.0.0"
}
```

### Aggregate Health Check

The web application aggregates all service health:

```rust
async fn health(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let (req, rx) = GetServiceStatus::new();
    state.service_coordinator.send(req).await;
    let status = rx.await?;

    let all_healthy = status.services
        .values()
        .all(|(state, _)| *state == ServiceState::Healthy);

    if all_healthy {
        (StatusCode::OK, Json(json!({"status": "healthy"})))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "status": "degraded",
            "services": status.services
        })))
    }
}
```

## Graceful Degradation

The framework supports graceful degradation when services are unavailable:

### Fallback Patterns

```rust
// Example: Fall back to local rate limiting if cache service unavailable
let rate_limit_result = match state.cache_client.check_rate_limit(&key).await {
    Ok(result) => result,
    Err(ServiceUnavailable) => {
        // Use local rate limiter agent
        let (req, rx) = CheckRateLimit::new(key.clone(), 1);
        state.local_rate_limiter.send(req).await;
        rx.await.unwrap_or_default()
    }
};
```

### Service Status in Templates

Display service status to users:

```html
{% if service_status.degraded %}
<div class="alert alert-warning">
    Some features may be temporarily unavailable
</div>
{% endif %}
```

## Monitoring

### Prometheus Metrics

Each service exports Prometheus metrics:

```bash
# Service metrics endpoint
curl http://localhost:50051/metrics
```

Common metrics:
- `grpc_requests_total` - Total gRPC requests
- `grpc_request_duration_seconds` - Request latency
- `service_health_status` - Health status (1=healthy, 0=unhealthy)
- `circuit_breaker_state` - Circuit breaker state

### Distributed Tracing

Enable OpenTelemetry tracing:

```toml
[observability]
otlp_endpoint = "http://jaeger:4317"
service_name = "myapp-web"
```

Traces propagate across service boundaries for request correlation.

## Security

### Service-to-Service Authentication

In production, enable mTLS between services:

```toml
[services]
tls_enabled = true
ca_cert = "/etc/certs/ca.crt"
client_cert = "/etc/certs/client.crt"
client_key = "/etc/certs/client.key"
```

### Network Policies

In Kubernetes, restrict service communication:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: web-to-services
spec:
  podSelector:
    matchLabels:
      app: web
  egress:
  - to:
    - podSelector:
        matchLabels:
          tier: services
    ports:
    - port: 50051
    - port: 50052
```

## Next Steps

- **[Docker Images](./15-docker-deployment.md)** - Containerizing services
- **[Kubernetes Deployment](./16-kubernetes.md)** - Production orchestration
- **[Performance Testing](./17-performance.md)** - Benchmarking services

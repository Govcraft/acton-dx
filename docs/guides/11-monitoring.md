# Monitoring Setup with Prometheus and Grafana

This guide shows how to set up comprehensive monitoring for acton-htmx applications using Prometheus and Grafana.

## Architecture

```
┌──────────────┐
│ Application  │ ──> /metrics (Prometheus format)
│ (acton-htmx) │
└──────────────┘
       ↓
┌──────────────┐
│  Prometheus  │ ──> Scrapes metrics, stores time-series data
└──────────────┘
       ↓
┌──────────────┐
│   Grafana    │ ──> Visualizes metrics, creates dashboards
└──────────────┘
```

## Quick Start

### 1. Application Metrics

Add health and metrics endpoints to your application:

```rust
use axum::{Router, routing::get};
use acton_htmx::health::{health_check, liveness, readiness};
use acton_htmx::observability::metrics::metrics_handler;

let app = Router::new()
    .route("/health", get(health_check))
    .route("/health/live", get(liveness))
    .route("/health/ready", get(readiness))
    .route("/metrics", get(metrics_handler));
```

### 2. Install Prometheus

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install prometheus

# Or download binary
wget https://github.com/prometheus/prometheus/releases/download/v2.45.0/prometheus-2.45.0.linux-amd64.tar.gz
tar xvf prometheus-2.45.0.linux-amd64.tar.gz
cd prometheus-2.45.0.linux-amd64
./prometheus --config.file=prometheus.yml
```

### 3. Install Grafana

```bash
# Ubuntu/Debian
sudo apt-get install -y software-properties-common
sudo add-apt-repository "deb https://packages.grafana.com/oss/deb stable main"
wget -q -O - https://packages.grafana.com/gpg.key | sudo apt-key add -
sudo apt-get update
sudo apt-get install grafana

# Start Grafana
sudo systemctl enable grafana-server
sudo systemctl start grafana-server
```

Access Grafana at `http://localhost:3000` (default: admin/admin)

## Prometheus Configuration

### Basic Configuration

Create `/etc/prometheus/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    environment: 'production'
    cluster: 'main'

# Alertmanager configuration
alerting:
  alertmanagers:
    - static_configs:
        - targets: ['localhost:9093']

# Load alert rules
rule_files:
  - '/etc/prometheus/alerts/*.yml'

# Scrape configurations
scrape_configs:
  # Prometheus itself
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']

  # acton-htmx application
  - job_name: 'acton-htmx'
    scrape_interval: 10s
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
      - target_label: job
        replacement: 'myapp'

  # Node exporter (system metrics)
  - job_name: 'node'
    static_configs:
      - targets: ['localhost:9100']

  # PostgreSQL exporter
  - job_name: 'postgres'
    static_configs:
      - targets: ['localhost:9187']

  # Redis exporter
  - job_name: 'redis'
    static_configs:
      - targets: ['localhost:9121']

  # Nginx exporter (if using nginx)
  - job_name: 'nginx'
    static_configs:
      - targets: ['localhost:9113']
```

### Service Discovery (Kubernetes)

For Kubernetes deployments:

```yaml
scrape_configs:
  - job_name: 'kubernetes-pods'
    kubernetes_sd_configs:
      - role: pod
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
        action: keep
        regex: true
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
        action: replace
        target_label: __metrics_path__
        regex: (.+)
      - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
        action: replace
        regex: ([^:]+)(?::\d+)?;(\d+)
        replacement: $1:$2
        target_label: __address__
```

### Multiple Instances

For load-balanced applications:

```yaml
scrape_configs:
  - job_name: 'acton-htmx-cluster'
    static_configs:
      - targets:
          - 'app1:8080'
          - 'app2:8080'
          - 'app3:8080'
        labels:
          cluster: 'main'
          environment: 'production'
```

## System Metrics Exporters

### Node Exporter (System Metrics)

Install and configure:

```bash
# Install
sudo apt install prometheus-node-exporter

# Enable and start
sudo systemctl enable prometheus-node-exporter
sudo systemctl start prometheus-node-exporter

# Metrics available at http://localhost:9100/metrics
```

Provides: CPU, memory, disk, network metrics.

### PostgreSQL Exporter

```bash
# Install
sudo apt install prometheus-postgres-exporter

# Configure
sudo nano /etc/default/prometheus-postgres-exporter

# Add:
DATA_SOURCE_NAME="postgresql://prometheus:password@localhost:5432/myapp?sslmode=disable"

# Restart
sudo systemctl restart prometheus-postgres-exporter

# Metrics at http://localhost:9187/metrics
```

### Redis Exporter

```bash
# Download
wget https://github.com/oliver006/redis_exporter/releases/download/v1.55.0/redis_exporter-v1.55.0.linux-amd64.tar.gz
tar xvf redis_exporter-v1.55.0.linux-amd64.tar.gz

# Run
./redis_exporter &

# Metrics at http://localhost:9121/metrics
```

### Nginx Exporter

```bash
# Install
sudo apt install prometheus-nginx-exporter

# Configure nginx stub_status
# Add to nginx.conf:
location /nginx_status {
    stub_status on;
    access_log off;
    allow 127.0.0.1;
    deny all;
}

# Start exporter
prometheus-nginx-exporter -nginx.scrape-uri http://localhost/nginx_status

# Metrics at http://localhost:9113/metrics
```

## Grafana Configuration

### Add Prometheus Data Source

1. Open Grafana: `http://localhost:3000`
2. Login (default: admin/admin)
3. Go to **Configuration → Data Sources**
4. Click **Add data source**
5. Select **Prometheus**
6. Configure:
   - Name: `Prometheus`
   - URL: `http://localhost:9090`
   - Access: `Server (default)`
7. Click **Save & Test**

### Import Dashboard

**Option 1: From Grafana.com**

1. Go to **Dashboards → Import**
2. Enter dashboard ID:
   - Node Exporter: `1860`
   - PostgreSQL: `9628`
   - Nginx: `12708`
3. Select Prometheus data source
4. Click **Import**

**Option 2: Custom Dashboard**

See [Grafana Dashboard Template](#grafana-dashboard-template) below.

## Application-Specific Metrics

### Custom Metrics in Application

```rust
use acton_htmx::observability::metrics::MetricsCollector;

// In your application state
pub struct AppState {
    metrics: Arc<MetricsCollector>,
    // ... other fields
}

// In middleware or handlers
state.metrics.inc_http_requests();
state.metrics.record_http_duration(duration_ms);
state.metrics.inc_jobs_enqueued();

// Expose metrics endpoint
async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    acton_htmx::observability::metrics::metrics_response(&state.metrics)
}
```

### Available Metrics

The `acton-htmx` framework exposes:

- `http_requests_total` - Total HTTP requests
- `http_request_duration_ms_total` - Total request duration
- `jobs_enqueued_total` - Background jobs enqueued
- `jobs_completed_total` - Jobs completed successfully
- `jobs_failed_total` - Jobs that failed
- `sessions_active` - Number of active sessions

## Alert Rules

Create `/etc/prometheus/alerts/application.yml`:

```yaml
groups:
  - name: application
    interval: 30s
    rules:
      # High error rate
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }} errors/sec on {{ $labels.instance }}"

      # Application down
      - alert: ApplicationDown
        expr: up{job="acton-htmx"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Application is down"
          description: "{{ $labels.instance }} has been down for more than 1 minute"

      # High response time
      - alert: HighResponseTime
        expr: rate(http_request_duration_ms_total[5m]) / rate(http_requests_total[5m]) > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High response time"
          description: "Average response time is {{ $value }}ms on {{ $labels.instance }}"

      # Job queue growing
      - alert: JobQueueGrowing
        expr: rate(jobs_enqueued_total[5m]) - rate(jobs_completed_total[5m]) > 10
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Job queue is growing"
          description: "Job queue is growing at {{ $value }} jobs/sec on {{ $labels.instance }}"

      # High job failure rate
      - alert: HighJobFailureRate
        expr: rate(jobs_failed_total[5m]) / rate(jobs_enqueued_total[5m]) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High job failure rate"
          description: "Job failure rate is {{ $value }}% on {{ $labels.instance }}"

  - name: system
    interval: 30s
    rules:
      # High CPU usage
      - alert: HighCPUUsage
        expr: 100 - (avg by(instance) (irate(node_cpu_seconds_total{mode="idle"}[5m])) * 100) > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High CPU usage"
          description: "CPU usage is {{ $value }}% on {{ $labels.instance }}"

      # High memory usage
      - alert: HighMemoryUsage
        expr: (1 - (node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes)) * 100 > 85
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
          description: "Memory usage is {{ $value }}% on {{ $labels.instance }}"

      # Disk space low
      - alert: DiskSpaceLow
        expr: (node_filesystem_avail_bytes{mountpoint="/"} / node_filesystem_size_bytes{mountpoint="/"}) * 100 < 15
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Disk space low"
          description: "Disk space is {{ $value }}% on {{ $labels.instance }}"

      # Database connection pool exhausted
      - alert: DatabasePoolExhausted
        expr: pg_stat_activity_count >= pg_settings_max_connections * 0.9
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Database connection pool exhausted"
          description: "{{ $value }} connections out of {{ pg_settings_max_connections }}"
```

## Alertmanager Configuration

Install Alertmanager:

```bash
sudo apt install prometheus-alertmanager
```

Configure `/etc/prometheus/alertmanager.yml`:

```yaml
global:
  resolve_timeout: 5m

route:
  group_by: ['alertname', 'cluster', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  receiver: 'email'
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'
      continue: true
    - match:
        severity: warning
      receiver: 'slack'

receivers:
  - name: 'email'
    email_configs:
      - to: 'alerts@example.com'
        from: 'prometheus@example.com'
        smarthost: 'smtp.example.com:587'
        auth_username: 'prometheus'
        auth_password: 'password'

  - name: 'slack'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK'
        channel: '#alerts'
        title: 'Alert: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'YOUR_PAGERDUTY_SERVICE_KEY'
```

## Grafana Dashboard Template

See `templates/grafana/acton-htmx-dashboard.json` in the project repository.

Key panels:
- HTTP Request Rate
- HTTP Response Time (p50, p95, p99)
- Error Rate
- Job Queue Size
- Job Processing Rate
- CPU Usage
- Memory Usage
- Database Connections
- Active Sessions

## Best Practices

1. **Set appropriate scrape intervals** - Balance between data granularity and storage
2. **Use recording rules** - Pre-compute expensive queries
3. **Set up alerting** - Monitor critical metrics
4. **Organize dashboards** - Separate by service/component
5. **Document metrics** - Add descriptions to custom metrics
6. **Monitor exporters** - Ensure exporters are running
7. **Secure endpoints** - Restrict access to /metrics
8. **Test alerts** - Verify alert routing works
9. **Monitor Prometheus** - Set up self-monitoring
10. **Backup configurations** - Keep configs in version control

## Related

- [Systemd Deployment](./08-systemd-deployment.md)
- [Reverse Proxy Setup](./09-reverse-proxy.md)
- [SSL/TLS Setup](./10-ssl-setup.md)
- [Troubleshooting Guide](./12-troubleshooting.md)

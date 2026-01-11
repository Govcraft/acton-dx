# Systemd Service Deployment

This guide shows how to deploy an `acton-htmx` application as a systemd service on Linux.

## Prerequisites

- Linux server with systemd (Ubuntu 20.04+, Debian 11+, CentOS 8+, etc.)
- Root or sudo access
- Compiled release binary
- PostgreSQL installed and running
- (Optional) Redis installed and running

## Building the Application

Build an optimized release binary:

```bash
cargo build --release
```

The binary will be in `target/release/<project-name>`.

## Deployment Structure

Recommended directory layout:

```
/opt/myapp/
├── bin/
│   └── myapp              # Release binary
├── config/
│   └── production.toml    # Configuration file
├── migrations/            # Database migrations
├── templates/             # Askama templates
├── static/               # Static assets
└── logs/                 # Application logs
```

## Setup Steps

### 1. Create Application User

Create a dedicated system user (no login, no home directory):

```bash
sudo useradd --system --no-create-home --shell /bin/false myapp
```

### 2. Copy Application Files

```bash
# Create directories
sudo mkdir -p /opt/myapp/{bin,config,migrations,templates,static,logs}

# Copy binary
sudo cp target/release/myapp /opt/myapp/bin/

# Copy configuration
sudo cp config/production.toml /opt/myapp/config/

# Copy migrations
sudo cp -r migrations/* /opt/myapp/migrations/

# Copy templates (if using)
sudo cp -r templates/* /opt/myapp/templates/

# Copy static files
sudo cp -r static/* /opt/myapp/static/

# Set ownership
sudo chown -R myapp:myapp /opt/myapp
sudo chmod +x /opt/myapp/bin/myapp
```

### 3. Create Environment File

Create `/etc/myapp/env` with environment variables:

```bash
sudo mkdir -p /etc/myapp
sudo touch /etc/myapp/env
sudo chmod 600 /etc/myapp/env
sudo chown myapp:myapp /etc/myapp/env
```

Edit `/etc/myapp/env`:

```bash
DATABASE_URL=postgresql://myapp:password@localhost:5432/myapp
REDIS_URL=redis://localhost:6379
SECRET_KEY=your-secret-key-here
RUST_LOG=info
```

**Security Note**: Keep this file secure (chmod 600) as it contains secrets.

### 4. Create Systemd Service File

Create `/etc/systemd/system/myapp.service`:

```ini
[Unit]
Description=My acton-htmx Application
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service

[Service]
Type=simple
User=myapp
Group=myapp
WorkingDirectory=/opt/myapp

# Environment file
EnvironmentFile=/etc/myapp/env

# Binary
ExecStart=/opt/myapp/bin/myapp

# Restart policy
Restart=always
RestartSec=5s

# Resource limits
LimitNOFILE=65536
MemoryLimit=1G

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/myapp/logs

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=myapp

[Install]
WantedBy=multi-user.target
```

### 5. Enable and Start Service

```bash
# Reload systemd to pick up new service file
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable myapp

# Start service
sudo systemctl start myapp

# Check status
sudo systemctl status myapp
```

## Service Management

### Common Commands

```bash
# Start service
sudo systemctl start myapp

# Stop service
sudo systemctl stop myapp

# Restart service
sudo systemctl restart myapp

# Reload configuration (graceful restart)
sudo systemctl reload myapp

# Check status
sudo systemctl status myapp

# View logs
sudo journalctl -u myapp -f

# View last 100 lines
sudo journalctl -u myapp -n 100

# View logs since yesterday
sudo journalctl -u myapp --since yesterday
```

### Logs

View application logs using `journalctl`:

```bash
# Follow logs (like tail -f)
sudo journalctl -u myapp -f

# Show last 1000 lines
sudo journalctl -u myapp -n 1000

# Show logs from last hour
sudo journalctl -u myapp --since "1 hour ago"

# Show logs with priority (error, warning, etc.)
sudo journalctl -u myapp -p err

# Export logs to file
sudo journalctl -u myapp > myapp.log
```

## Advanced Configuration

### Multiple Worker Processes

For CPU-intensive workloads, run multiple instances:

```ini
# /etc/systemd/system/myapp@.service
[Unit]
Description=My acton-htmx Application (Instance %i)
After=network.target postgresql.service

[Service]
Type=simple
User=myapp
Group=myapp
WorkingDirectory=/opt/myapp
EnvironmentFile=/etc/myapp/env
Environment="PORT=808%i"
ExecStart=/opt/myapp/bin/myapp
Restart=always

[Install]
WantedBy=multi-user.target
```

Start multiple instances:

```bash
sudo systemctl start myapp@0
sudo systemctl start myapp@1
sudo systemctl start myapp@2
sudo systemctl start myapp@3
```

Use nginx to load balance across instances (ports 8080-8083).

### Resource Limits

Add to `[Service]` section:

```ini
# CPU quota (50% of one core)
CPUQuota=50%

# Memory limit (1GB)
MemoryLimit=1G
MemoryHigh=900M

# Max open files
LimitNOFILE=65536

# Max processes
LimitNPROC=512
```

### Security Hardening

Enhanced security options for `[Service]`:

```ini
# Run as nobody if binary doesn't need file write
User=nobody
Group=nogroup

# Prevent privilege escalation
NoNewPrivileges=true

# Private /tmp
PrivateTmp=true

# Read-only filesystem (except specified paths)
ProtectSystem=strict
ReadWritePaths=/opt/myapp/logs /opt/myapp/uploads

# No access to /home
ProtectHome=true

# Hide /proc
ProtectProc=invisible

# Prevent access to kernel logs
ProtectKernelLogs=true

# Prevent loading kernel modules
ProtectKernelModules=true

# Restrict real-time
RestrictRealtime=true

# Namespace isolation
PrivateDevices=true
PrivateUsers=true
```

## Zero-Downtime Deployment

### 1. Build New Binary

```bash
cargo build --release
```

### 2. Upload to Server

```bash
scp target/release/myapp user@server:/tmp/myapp-new
```

### 3. Replace Binary

```bash
# SSH to server
ssh user@server

# Stop service
sudo systemctl stop myapp

# Backup old binary
sudo cp /opt/myapp/bin/myapp /opt/myapp/bin/myapp.old

# Install new binary
sudo cp /tmp/myapp-new /opt/myapp/bin/myapp
sudo chown myapp:myapp /opt/myapp/bin/myapp
sudo chmod +x /opt/myapp/bin/myapp

# Start service
sudo systemctl start myapp

# Check status
sudo systemctl status myapp
```

### 4. Rollback if Needed

```bash
# Stop service
sudo systemctl stop myapp

# Restore old binary
sudo cp /opt/myapp/bin/myapp.old /opt/myapp/bin/myapp

# Start service
sudo systemctl start myapp
```

## Health Checks

Add health check to systemd service:

```ini
[Service]
# ... other settings ...

# Health check script
ExecStartPost=/usr/bin/sleep 5
ExecStartPost=/usr/bin/curl -f http://localhost:8080/health || exit 1
```

Or create a separate timer for periodic health checks:

```ini
# /etc/systemd/system/myapp-health.service
[Unit]
Description=My App Health Check
After=myapp.service

[Service]
Type=oneshot
ExecStart=/usr/bin/curl -f http://localhost:8080/health
```

```ini
# /etc/systemd/system/myapp-health.timer
[Unit]
Description=My App Health Check Timer

[Timer]
OnBootSec=1min
OnUnitActiveSec=1min

[Install]
WantedBy=timers.target
```

Enable the timer:

```bash
sudo systemctl enable myapp-health.timer
sudo systemctl start myapp-health.timer
```

## Monitoring

### Prometheus Node Exporter

Install node exporter for system metrics:

```bash
sudo apt install prometheus-node-exporter
```

Metrics available at: `http://localhost:9100/metrics`

### Application Metrics

Expose application metrics at `/metrics` endpoint (using `acton-htmx` metrics):

```rust
use acton_htmx::observability::metrics::metrics_handler;

let app = Router::new()
    .route("/metrics", get(metrics_handler));
```

Configure Prometheus to scrape:

```yaml
# /etc/prometheus/prometheus.yml
scrape_configs:
  - job_name: 'myapp'
    static_configs:
      - targets: ['localhost:8080']
```

## Troubleshooting

### Service Won't Start

```bash
# Check service status
sudo systemctl status myapp

# View logs
sudo journalctl -u myapp -n 100

# Check for errors
sudo journalctl -u myapp -p err

# Verify binary is executable
ls -l /opt/myapp/bin/myapp

# Test binary manually
sudo -u myapp /opt/myapp/bin/myapp
```

### Permission Errors

```bash
# Check file ownership
ls -la /opt/myapp/

# Fix ownership
sudo chown -R myapp:myapp /opt/myapp

# Check database permissions
sudo -u postgres psql -c "\du myapp"
```

### High Memory Usage

```bash
# Check memory usage
sudo systemctl status myapp

# View detailed memory
sudo systemd-cgtop

# Set memory limit
sudo systemctl edit myapp

# Add:
[Service]
MemoryLimit=1G
```

### Service Keeps Restarting

```bash
# View crash logs
sudo journalctl -u myapp --since "10 minutes ago"

# Check restart count
systemctl show myapp | grep NRestarts

# Disable auto-restart temporarily
sudo systemctl edit myapp

# Add:
[Service]
Restart=no
```

## Best Practices

1. **Always use a dedicated user** - Never run as root
2. **Set resource limits** - Prevent resource exhaustion
3. **Enable security hardening** - Use ProtectSystem, PrivateTmp, etc.
4. **Monitor logs** - Use journalctl regularly
5. **Health checks** - Implement /health endpoint
6. **Backup binaries** - Keep previous version for rollback
7. **Test deployments** - Run in staging first
8. **Document procedures** - Keep runbook up to date

## Related

- [Nginx/Caddy Reverse Proxy](./09-reverse-proxy.md)
- [SSL/TLS Certificates](./10-ssl-setup.md)
- [Monitoring Setup](./11-monitoring.md)
- [Docker Deployment](./05-deployment.md#docker-deployment)

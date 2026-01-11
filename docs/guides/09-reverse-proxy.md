# Reverse Proxy Setup

This guide covers setting up Nginx or Caddy as a reverse proxy for an `acton-htmx` application.

## Why Use a Reverse Proxy?

- **SSL/TLS termination** - Handle HTTPS traffic
- **Load balancing** - Distribute traffic across multiple instances
- **Static file serving** - Serve assets efficiently
- **Compression** - Gzip/Brotli compression
- **Rate limiting** - Protect against abuse
- **Caching** - Improve performance

## Nginx Configuration

### Installation

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install nginx

# CentOS/RHEL
sudo yum install nginx
```

### Basic Configuration

Create `/etc/nginx/sites-available/myapp`:

```nginx
# Upstream backend servers
upstream myapp_backend {
    # Single server
    server 127.0.0.1:8080;

    # Multiple servers for load balancing
    # server 127.0.0.1:8080;
    # server 127.0.0.1:8081;
    # server 127.0.0.1:8082;

    # Load balancing method
    # least_conn;  # Least connections
    # ip_hash;     # Sticky sessions

    keepalive 32;
}

# HTTP server (redirect to HTTPS)
server {
    listen 80;
    listen [::]:80;
    server_name example.com www.example.com;

    # Redirect all HTTP to HTTPS
    return 301 https://$server_name$request_uri;
}

# HTTPS server
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name example.com www.example.com;

    # SSL certificates (Let's Encrypt)
    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    # SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384';
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    # CSP (adjust for your needs)
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline' cdn.jsdelivr.net; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' data:;" always;

    # Gzip compression
    gzip on;
    gzip_vary on;
    gzip_proxied any;
    gzip_comp_level 6;
    gzip_types text/plain text/css text/xml text/javascript application/json application/javascript application/xml+rss application/rss+xml font/truetype font/opentype application/vnd.ms-fontobject image/svg+xml;

    # Rate limiting zones (defined globally)
    limit_req zone=general burst=20 nodelay;

    # Static files
    location /static/ {
        alias /opt/myapp/static/;
        expires 1y;
        add_header Cache-Control "public, immutable";
        access_log off;
    }

    location /uploads/ {
        alias /opt/myapp/uploads/;
        expires 30d;
        add_header Cache-Control "public";
    }

    # Health check endpoint (no rate limit)
    location /health {
        proxy_pass http://myapp_backend;
        access_log off;
    }

    # Metrics endpoint (restrict access)
    location /metrics {
        allow 127.0.0.1;       # Prometheus server
        allow 10.0.0.0/8;      # Internal network
        deny all;

        proxy_pass http://myapp_backend;
        access_log off;
    }

    # Application endpoints
    location / {
        proxy_pass http://myapp_backend;
        proxy_http_version 1.1;

        # Proxy headers
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Forwarded-Host $host;
        proxy_set_header X-Forwarded-Port $server_port;

        # WebSocket support (if needed)
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";

        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;

        # Buffering
        proxy_buffering on;
        proxy_buffer_size 4k;
        proxy_buffers 8 4k;
        proxy_busy_buffers_size 8k;
    }

    # Error pages
    error_page 502 503 504 /50x.html;
    location = /50x.html {
        root /usr/share/nginx/html;
    }
}
```

### Global Rate Limiting

Add to `/etc/nginx/nginx.conf` in `http` block:

```nginx
http {
    # ... other settings ...

    # Rate limiting zones
    limit_req_zone $binary_remote_addr zone=general:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=auth:10m rate=5r/m;
    limit_req_status 429;

    # Connection limits
    limit_conn_zone $binary_remote_addr zone=addr:10m;
    limit_conn addr 10;
}
```

### Enable Configuration

```bash
# Create symlink
sudo ln -s /etc/nginx/sites-available/myapp /etc/nginx/sites-enabled/

# Test configuration
sudo nginx -t

# Reload nginx
sudo systemctl reload nginx
```

### Authentication-Specific Rate Limiting

Add to specific locations:

```nginx
location /login {
    limit_req zone=auth burst=3 nodelay;
    proxy_pass http://myapp_backend;
}

location /register {
    limit_req zone=auth burst=3 nodelay;
    proxy_pass http://myapp_backend;
}
```

## Caddy Configuration

Caddy is simpler to configure and handles SSL automatically via Let's Encrypt.

### Installation

```bash
# Ubuntu/Debian
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update
sudo apt install caddy
```

### Basic Configuration

Create `/etc/caddy/Caddyfile`:

```caddy
# Automatic HTTPS with Let's Encrypt
example.com www.example.com {
    # Reverse proxy to backend
    reverse_proxy localhost:8080 {
        # Health check
        health_uri /health
        health_interval 10s
        health_timeout 5s
    }

    # Static files
    handle /static/* {
        root * /opt/myapp/static
        file_server {
            precompressed gzip
        }
    }

    handle /uploads/* {
        root * /opt/myapp/uploads
        file_server
    }

    # Security headers
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains"
        X-Frame-Options "SAMEORIGIN"
        X-Content-Type-Options "nosniff"
        X-XSS-Protection "1; mode=block"
        Referrer-Policy "strict-origin-when-cross-origin"
        Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline' cdn.jsdelivr.net; style-src 'self' 'unsafe-inline';"
    }

    # Compression
    encode gzip zstd

    # Logging
    log {
        output file /var/log/caddy/access.log
        format json
    }
}
```

### Load Balancing with Caddy

```caddy
example.com {
    reverse_proxy localhost:8080 localhost:8081 localhost:8082 {
        # Load balancing policy
        lb_policy least_conn  # or: round_robin, ip_hash, first, etc.

        # Health checks
        health_uri /health
        health_interval 10s
        health_timeout 5s

        # Retry failed requests
        lb_retries 3
    }
}
```

### Enable and Start

```bash
# Enable and start Caddy
sudo systemctl enable caddy
sudo systemctl start caddy

# Check status
sudo systemctl status caddy

# View logs
sudo journalctl -u caddy -f
```

## SSL/TLS Certificates

### Let's Encrypt with Certbot (Nginx)

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Obtain certificate (interactive)
sudo certbot --nginx -d example.com -d www.example.com

# Test auto-renewal
sudo certbot renew --dry-run

# Auto-renewal is configured via systemd timer
sudo systemctl list-timers | grep certbot
```

### Let's Encrypt with Caddy

Caddy handles SSL automatically! Just specify your domain:

```caddy
example.com {
    # Caddy automatically obtains and renews certificates
    reverse_proxy localhost:8080
}
```

### Self-Signed Certificate (Development)

```bash
# Generate self-signed certificate
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout /etc/ssl/private/selfsigned.key \
    -out /etc/ssl/certs/selfsigned.crt \
    -subj "/CN=localhost"

# Use in nginx
ssl_certificate /etc/ssl/certs/selfsigned.crt;
ssl_certificate_key /etc/ssl/private/selfsigned.key;
```

## Monitoring

### Nginx Status Module

Enable stub_status module:

```nginx
server {
    listen 127.0.0.1:8081;

    location /nginx_status {
        stub_status on;
        access_log off;
        allow 127.0.0.1;
        deny all;
    }
}
```

Access: `http://127.0.0.1:8081/nginx_status`

### Caddy Metrics

Caddy exposes Prometheus metrics:

```caddy
{
    servers {
        metrics
    }
}
```

Metrics available at: `http://localhost:2019/metrics`

## Troubleshooting

### Nginx

```bash
# Test configuration
sudo nginx -t

# View error log
sudo tail -f /var/log/nginx/error.log

# Check if nginx is running
sudo systemctl status nginx

# Reload configuration
sudo systemctl reload nginx
```

### Caddy

```bash
# Validate configuration
sudo caddy validate --config /etc/caddy/Caddyfile

# View logs
sudo journalctl -u caddy -f

# Check if Caddy is running
sudo systemctl status caddy

# Reload configuration
sudo systemctl reload caddy
```

### Common Issues

**502 Bad Gateway**:
- Backend server not running
- Incorrect proxy_pass URL
- Firewall blocking connection

**504 Gateway Timeout**:
- Backend taking too long to respond
- Increase proxy timeout values
- Check backend performance

**Certificate Errors**:
- Ensure DNS points to server
- Check firewall allows port 80 (for Let's Encrypt validation)
- Verify certificate paths are correct

## Best Practices

1. **Always use HTTPS** - Redirect HTTP to HTTPS
2. **Enable compression** - Gzip or Brotli for text content
3. **Set security headers** - HSTS, CSP, X-Frame-Options, etc.
4. **Rate limiting** - Protect auth endpoints
5. **Health checks** - Monitor backend availability
6. **Serve static files** - Use reverse proxy, not application
7. **Enable logging** - Monitor access and errors
8. **Regular updates** - Keep nginx/Caddy updated

## Related

- [Systemd Deployment](./08-systemd-deployment.md)
- [SSL/TLS Setup](./10-ssl-setup.md)
- [Monitoring Setup](./11-monitoring.md)

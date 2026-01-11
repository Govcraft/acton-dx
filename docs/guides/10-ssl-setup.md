# SSL/TLS Certificate Setup

This guide covers setting up SSL/TLS certificates for secure HTTPS connections.

## Quick Start

**Recommended**: Use Caddy for automatic SSL management:

```caddy
# /etc/caddy/Caddyfile
example.com {
    reverse_proxy localhost:8080
}
```

That's it! Caddy automatically obtains and renews certificates from Let's Encrypt.

## Let's Encrypt with Certbot (Nginx/Apache)

### Installation

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install certbot python3-certbot-nginx

# CentOS/RHEL
sudo yum install certbot python3-certbot-nginx
```

### Obtain Certificate

```bash
# For Nginx (automatic configuration)
sudo certbot --nginx -d example.com -d www.example.com

# For Apache (automatic configuration)
sudo certbot --apache -d example.com -d www.example.com

# Manual (webroot)
sudo certbot certonly --webroot -w /var/www/html -d example.com -d www.example.com

# Standalone (requires port 80 free)
sudo certbot certonly --standalone -d example.com -d www.example.com
```

### Certificate Locations

Certificates are stored in `/etc/letsencrypt/live/example.com/`:

- `fullchain.pem` - Full certificate chain
- `privkey.pem` - Private key
- `cert.pem` - Domain certificate only
- `chain.pem` - Intermediate certificates

### Auto-Renewal

Let's Encrypt certificates expire after 90 days. Certbot sets up auto-renewal:

```bash
# Test renewal (dry run)
sudo certbot renew --dry-run

# Manual renewal
sudo certbot renew

# Check renewal timer
sudo systemctl list-timers | grep certbot
sudo systemctl status certbot.timer
```

Renewal happens automatically via systemd timer.

## Manual Nginx Configuration

If you obtained certificates manually, configure Nginx:

```nginx
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name example.com www.example.com;

    # SSL certificates
    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    # Modern SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384';
    ssl_prefer_server_ciphers off;

    # SSL session caching
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;
    ssl_session_tickets off;

    # OCSP stapling
    ssl_stapling on;
    ssl_stapling_verify on;
    ssl_trusted_certificate /etc/letsencrypt/live/example.com/chain.pem;
    resolver 8.8.8.8 8.8.4.4 valid=300s;
    resolver_timeout 5s;

    # HSTS
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;

    # ... rest of configuration
}
```

## Wildcard Certificates

For `*.example.com`:

```bash
# Requires DNS-01 challenge (manual or with DNS provider plugin)
sudo certbot certonly --manual --preferred-challenges dns -d "*.example.com" -d example.com

# Follow instructions to add TXT record to DNS
```

DNS provider plugins (automatic):

```bash
# Cloudflare
sudo apt install python3-certbot-dns-cloudflare
sudo certbot certonly --dns-cloudflare --dns-cloudflare-credentials ~/.secrets/cloudflare.ini -d "*.example.com" -d example.com

# Route53
sudo apt install python3-certbot-dns-route53
sudo certbot certonly --dns-route53 -d "*.example.com" -d example.com
```

## Self-Signed Certificates (Development)

For local development:

```bash
# Generate private key
openssl genrsa -out localhost.key 2048

# Generate certificate signing request
openssl req -new -key localhost.key -out localhost.csr \
    -subj "/C=US/ST=State/L=City/O=Organization/CN=localhost"

# Generate self-signed certificate (1 year validity)
openssl x509 -req -days 365 -in localhost.csr -signkey localhost.key -out localhost.crt

# Or combine in one step
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout localhost.key -out localhost.crt \
    -subj "/CN=localhost"
```

Add to Nginx:

```nginx
ssl_certificate /etc/ssl/certs/localhost.crt;
ssl_certificate_key /etc/ssl/private/localhost.key;
```

**Note**: Browsers will show a warning for self-signed certificates. This is normal for development.

## Custom Certificate Authority (CA)

For internal services:

### 1. Create CA

```bash
# Generate CA private key
openssl genrsa -out ca.key 4096

# Generate CA certificate
openssl req -new -x509 -days 3650 -key ca.key -out ca.crt \
    -subj "/CN=My CA/O=My Organization"
```

### 2. Create Server Certificate

```bash
# Generate server private key
openssl genrsa -out server.key 2048

# Generate certificate signing request
openssl req -new -key server.key -out server.csr \
    -subj "/CN=example.com"

# Sign with CA
openssl x509 -req -days 365 -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt
```

### 3. Trust CA

On client machines:

```bash
# Ubuntu/Debian
sudo cp ca.crt /usr/local/share/ca-certificates/my-ca.crt
sudo update-ca-certificates

# CentOS/RHEL
sudo cp ca.crt /etc/pki/ca-trust/source/anchors/my-ca.crt
sudo update-ca-trust
```

## SSL Configuration Best Practices

### Mozilla SSL Configuration Generator

Use [https://ssl-config.mozilla.org/](https://ssl-config.mozilla.org/) for modern SSL configs.

### Recommended Nginx Config

```nginx
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384';
ssl_prefer_server_ciphers off;
ssl_session_cache shared:SSL:10m;
ssl_session_tickets off;
ssl_stapling on;
ssl_stapling_verify on;
```

### HTTP Strict Transport Security (HSTS)

Force HTTPS for all subdomains:

```nginx
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;
```

Submit to HSTS preload list: [https://hstspreload.org/](https://hstspreload.org/)

## Testing SSL Configuration

### Online Tools

- [SSL Labs](https://www.ssllabs.com/ssltest/) - Comprehensive SSL test (A+ rating recommended)
- [SSL Checker](https://www.sslshopper.com/ssl-checker.html) - Quick certificate verification
- [Security Headers](https://securityheaders.com/) - Check security headers

### Command Line

```bash
# Test SSL connection
openssl s_client -connect example.com:443 -servername example.com

# Check certificate expiry
echo | openssl s_client -connect example.com:443 2>/dev/null | openssl x509 -noout -dates

# Test specific TLS version
openssl s_client -connect example.com:443 -tls1_2
openssl s_client -connect example.com:443 -tls1_3

# Check certificate chain
openssl s_client -showcerts -connect example.com:443
```

## Troubleshooting

### Certificate Not Valid

**Issue**: Browser shows "Certificate not valid" error

**Solutions**:
- Verify certificate matches domain name
- Check certificate expiration date
- Ensure full chain is configured (fullchain.pem, not cert.pem)
- Verify intermediate certificates are included

### Let's Encrypt Rate Limits

**Issue**: "too many certificates already issued" error

**Solutions**:
- Wait 7 days for rate limit reset
- Use `--staging` flag for testing
- Combine multiple domains in one certificate
- Check rate limits: [https://letsencrypt.org/docs/rate-limits/](https://letsencrypt.org/docs/rate-limits/)

### DNS Validation Fails

**Issue**: DNS-01 challenge fails for wildcard certificates

**Solutions**:
- Verify TXT record is propagated (use `dig TXT _acme-challenge.example.com`)
- Wait 5-10 minutes for DNS propagation
- Use correct DNS zone (may need to add to apex domain)
- Check DNS provider API credentials

### Mixed Content Warnings

**Issue**: HTTPS page loading HTTP resources

**Solutions**:
- Use relative URLs (`/static/style.css` instead of `http://...`)
- Use protocol-relative URLs (`//cdn.example.com/script.js`)
- Update all external resources to HTTPS
- Check Content-Security-Policy header

## Monitoring Certificate Expiry

### Automated Monitoring

```bash
#!/bin/bash
# check-ssl-expiry.sh

DOMAIN="example.com"
DAYS_WARNING=30

EXPIRY=$(echo | openssl s_client -servername $DOMAIN -connect $DOMAIN:443 2>/dev/null | openssl x509 -noout -enddate | cut -d= -f2)
EXPIRY_EPOCH=$(date -d "$EXPIRY" +%s)
NOW_EPOCH=$(date +%s)
DAYS_LEFT=$(( ($EXPIRY_EPOCH - $NOW_EPOCH) / 86400 ))

if [ $DAYS_LEFT -lt $DAYS_WARNING ]; then
    echo "WARNING: SSL certificate for $DOMAIN expires in $DAYS_LEFT days!"
    # Send alert (email, slack, etc.)
fi
```

### Prometheus Monitoring

Use `blackbox_exporter` to monitor certificate expiry:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'blackbox'
    metrics_path: /probe
    params:
      module: [http_2xx]
    static_configs:
      - targets:
        - https://example.com
    relabel_configs:
      - source_labels: [__address__]
        target_label: __param_target
      - source_labels: [__param_target]
        target_label: instance
      - target_label: __address__
        replacement: localhost:9115
```

## Related

- [Reverse Proxy Setup](./09-reverse-proxy.md)
- [Systemd Deployment](./08-systemd-deployment.md)
- [Monitoring Setup](./11-monitoring.md)

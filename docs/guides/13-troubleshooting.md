# Troubleshooting Guide

Common issues and solutions for acton-htmx applications in production.

## Application Won't Start

### Symptoms
- Service fails to start
- Immediate crash on startup
- "Address already in use" error

### Solutions

**Check if port is already in use**:
```bash
sudo lsof -i :8080
# or
sudo netstat -tulpn | grep 8080
```

**Kill process using port**:
```bash
sudo kill -9 <PID>
```

**Check configuration**:
```bash
# Verify config file exists
cat /opt/myapp/config/production.toml

# Check environment variables
sudo systemctl cat myapp | grep Environment

# Test binary manually
sudo -u myapp /opt/myapp/bin/myapp
```

**Check logs**:
```bash
sudo journalctl -u myapp -n 100
sudo journalctl -u myapp --since "10 minutes ago"
```

**Common issues**:
- Missing environment variables
- Invalid database connection string
- Missing config file
- Permission errors
- Binary not executable

## High Memory Usage

### Symptoms
- Memory usage constantly increasing
- OOM killer terminating process
- System becoming unresponsive

### Solutions

**Check current memory usage**:
```bash
# Application memory
sudo systemctl status myapp

# System memory
free -h
top -o %MEM
```

**Set memory limits** (in systemd service):
```ini
[Service]
MemoryLimit=1G
MemoryHigh=900M
```

**Find memory leaks**:
```bash
# Use valgrind (development)
valgrind --leak-check=full ./myapp

# Monitor heap allocations
heaptrack ./myapp
```

**Common causes**:
- Connection pool leaks (not returning connections)
- Unbounded caches growing
- Large file uploads
- Session storage not cleaning up
- Background job queue growing

**Solutions**:
- Implement connection pool timeouts
- Add cache eviction policies
- Limit file upload sizes
- Configure session expiration
- Process background jobs faster

## High CPU Usage

### Symptoms
- CPU usage consistently above 80%
- Slow response times
- Requests timing out

### Solutions

**Identify CPU-intensive operations**:
```bash
# Top processes
top
htop

# Application CPU usage
sudo systemctl status myapp

# Profile with perf
perf record -F 99 -p <PID> -g -- sleep 60
perf report
```

**Common causes**:
- Inefficient database queries
- N+1 query problems
- Large computations in request handlers
- Slow template rendering
- Infinite loops or runaway threads

**Solutions**:
- Optimize database queries (add indexes)
- Use connection pooling
- Move heavy operations to background jobs
- Cache expensive computations
- Add query timeouts

**Limit CPU usage** (systemd):
```ini
[Service]
CPUQuota=50%  # Limit to 50% of one core
```

## Database Connection Errors

### Symptoms
- "Connection refused" errors
- "Too many connections" errors
- Slow database queries

### Solutions

**Check database is running**:
```bash
sudo systemctl status postgresql
# or for docker
docker ps | grep postgres
```

**Test connection**:
```bash
psql -h localhost -U myapp -d myapp
```

**Check connection pool settings**:
```toml
# config/production.toml
[database]
max_connections = 20
min_connections = 5
connect_timeout = 30
idle_timeout = 600
```

**Check database connections**:
```sql
-- PostgreSQL
SELECT count(*) FROM pg_stat_activity;
SELECT * FROM pg_stat_activity WHERE datname = 'myapp';

-- Show max connections
SHOW max_connections;
```

**Increase database connection limit**:
```bash
# Edit postgresql.conf
sudo nano /etc/postgresql/14/main/postgresql.conf

# Change:
max_connections = 200  # (was 100)

# Restart PostgreSQL
sudo systemctl restart postgresql
```

**Common issues**:
- Connection pool exhausted
- Network connectivity issues
- Database not running
- Wrong credentials
- Firewall blocking connection

## Slow Response Times

### Symptoms
- Requests taking > 1 second
- Users complaining about slowness
- Timeouts in browser

### Solutions

**Measure response times**:
```bash
# Using curl
time curl -I https://example.com

# Using wrk (load testing)
wrk -t12 -c400 -d30s https://example.com
```

**Check Prometheus metrics**:
```promql
# Average response time
rate(http_request_duration_ms_total[5m]) / rate(http_requests_total[5m])

# 95th percentile
histogram_quantile(0.95, rate(http_request_duration_bucket[5m]))
```

**Profile application**:
```bash
# Using flamegraph
cargo flamegraph --bin myapp
```

**Common causes**:
- Slow database queries
- N+1 query problems
- Missing database indexes
- Large template rendering
- Slow external API calls
- Network latency

**Solutions**:
- Add database indexes
- Use query result caching
- Implement pagination
- Optimize templates
- Use async/await for I/O
- Add connection pooling
- Use CDN for static files

## 502 Bad Gateway

### Symptoms
- Nginx showing 502 error
- Application unreachable via reverse proxy
- Works when accessing directly

### Solutions

**Check application is running**:
```bash
sudo systemctl status myapp
curl http://localhost:8080/health
```

**Check Nginx configuration**:
```bash
sudo nginx -t
sudo tail -f /var/log/nginx/error.log
```

**Check upstream connection**:
```nginx
# Verify proxy_pass URL is correct
upstream myapp_backend {
    server 127.0.0.1:8080;  # Correct port?
}
```

**Check logs**:
```bash
# Application logs
sudo journalctl -u myapp -n 100

# Nginx error log
sudo tail -f /var/log/nginx/error.log
```

**Common issues**:
- Application not running
- Wrong proxy_pass port
- Firewall blocking connection
- Application crashed
- SELinux blocking connection

## SSL Certificate Errors

### Symptoms
- "Certificate not valid" warnings
- HTTPS not working
- Mixed content warnings

### Solutions

**Check certificate validity**:
```bash
echo | openssl s_client -connect example.com:443 2>/dev/null | openssl x509 -noout -dates
```

**Renew Let's Encrypt certificate**:
```bash
sudo certbot renew
sudo systemctl reload nginx
```

**Check certificate chain**:
```bash
openssl s_client -showcerts -connect example.com:443
```

**Test SSL configuration**:
- Use [SSL Labs](https://www.ssllabs.com/ssltest/)
- Verify certificate matches domain
- Check intermediate certificates are included

**Common issues**:
- Expired certificate
- Certificate doesn't match domain
- Missing intermediate certificates
- Using cert.pem instead of fullchain.pem
- Certificate not readable by nginx

## Session Issues

### Symptoms
- Users logged out unexpectedly
- Session data lost
- "CSRF token mismatch" errors

### Solutions

**Check session configuration**:
```toml
[session]
cookie_name = "session_id"
max_age_seconds = 604800  # 7 days
secure = true
http_only = true
same_site = "Lax"
```

**Check Redis connection** (if using Redis sessions):
```bash
redis-cli ping
redis-cli info | grep connected_clients
```

**Check CSRF middleware**:
```rust
// Ensure CSRF middleware is added
.layer(CsrfLayer::new(&state))
```

**Common issues**:
- Session cookie not being sent (check `secure` flag in dev)
- Redis not running (if using Redis sessions)
- Session expiration too short
- CSRF tokens not matching (check form templates)
- Cookie domain mismatch

## Background Jobs Not Processing

### Symptoms
- Jobs enqueued but not completing
- Job queue growing
- No job processing activity

### Solutions

**Check job metrics**:
```promql
# Jobs enqueued vs completed
rate(jobs_enqueued_total[5m])
rate(jobs_completed_total[5m])

# Failed jobs
rate(jobs_failed_total[5m])
```

**Check job agent is running**:
```bash
# Look for job processing in logs
sudo journalctl -u myapp | grep "job"
```

**Check Redis connection** (if using Redis persistence):
```bash
redis-cli ping
redis-cli llen job_queue
```

**Common issues**:
- Job agent not started
- Job worker pool exhausted
- Database connection issues
- Jobs failing silently
- Redis connection lost

**Solutions**:
- Increase worker pool size
- Add job retry logic
- Fix underlying job failures
- Monitor job execution time

## File Upload Issues

### Symptoms
- Uploads fail
- "413 Payload Too Large" errors
- Files not saving

### Solutions

**Check upload size limits**:
```nginx
# Nginx
client_max_body_size 100M;
```

```rust
// Application
.layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // 100MB
```

**Check file permissions**:
```bash
ls -la /opt/myapp/uploads/
sudo chown -R myapp:myapp /opt/myapp/uploads/
sudo chmod -R 755 /opt/myapp/uploads/
```

**Check disk space**:
```bash
df -h
```

**Common issues**:
- Upload size limit too small
- Insufficient disk space
- Permission errors
- Path doesn't exist
- MIME type validation failing

## Email Not Sending

### Symptoms
- Emails not being delivered
- SMTP errors in logs
- Silent failures

### Solutions

**Check SMTP configuration**:
```toml
[email]
smtp_host = "smtp.gmail.com"
smtp_port = 587
smtp_username = "your-email@gmail.com"
smtp_password = "your-app-password"  # Use environment variable!
```

**Test SMTP connection**:
```bash
telnet smtp.gmail.com 587
```

**Check logs for errors**:
```bash
sudo journalctl -u myapp | grep -i email
sudo journalctl -u myapp | grep -i smtp
```

**Common issues**:
- Wrong SMTP credentials
- Firewall blocking port 587
- Using regular password instead of app password
- Email marked as spam
- Rate limiting by email provider

**Solutions**:
- Use app-specific passwords (Gmail, etc.)
- Check SPF/DKIM records
- Verify sender domain
- Add to email whitelist

## Performance Degradation Over Time

### Symptoms
- Application gets slower over time
- Requires periodic restarts
- Memory usage gradually increasing

### Solutions

**Check for memory leaks**:
```bash
# Monitor memory over time
watch -n 5 'systemctl status myapp | grep Memory'

# Check heap
heaptrack ./myapp
```

**Check connection pool leaks**:
```sql
-- PostgreSQL
SELECT count(*), state
FROM pg_stat_activity
GROUP BY state;
```

**Check for unbounded caches**:
- Implement cache eviction
- Set maximum cache size
- Use LRU caching strategy

**Common causes**:
- Connection leaks
- Session data not expiring
- Cache growing unbounded
- Background job queue growing
- File descriptors not being closed

## Debugging Tools

### Log Analysis
```bash
# Real-time logs
sudo journalctl -u myapp -f

# Last 1000 lines
sudo journalctl -u myapp -n 1000

# Errors only
sudo journalctl -u myapp -p err

# Since yesterday
sudo journalctl -u myapp --since yesterday

# Export to file
sudo journalctl -u myapp > myapp.log
```

### System Metrics
```bash
# CPU and memory
top
htop

# Disk usage
df -h
du -sh /opt/myapp/*

# Network connections
netstat -tulpn
ss -tulpn

# Open files
lsof -p <PID>
```

### Database Queries
```sql
-- PostgreSQL slow queries
SELECT query, calls, total_time, mean_time
FROM pg_stat_statements
ORDER BY mean_time DESC
LIMIT 10;

-- Active connections
SELECT * FROM pg_stat_activity;

-- Database size
SELECT pg_size_pretty(pg_database_size('myapp'));

-- Table sizes
SELECT
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
```

## Getting Help

When opening a support ticket or issue, include:

1. **Version information**:
   ```bash
   ./myapp --version
   cargo --version
   rustc --version
   ```

2. **Error messages** from logs

3. **Configuration** (redact secrets):
   ```bash
   cat config/production.toml
   ```

4. **System information**:
   ```bash
   uname -a
   cat /etc/os-release
   ```

5. **Steps to reproduce** the issue

6. **Expected vs actual behavior**

## Related

- [Production Checklist](./12-production-checklist.md)
- [Monitoring Setup](./11-monitoring.md)
- [Systemd Deployment](./08-systemd-deployment.md)

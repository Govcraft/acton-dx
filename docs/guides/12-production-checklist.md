# Production Deployment Checklist

Complete this checklist before deploying your acton-htmx application to production.

## Pre-Deployment

### Code Quality
- [ ] All tests passing (`cargo test`)
- [ ] Zero clippy warnings (`cargo clippy --all-targets -- -D warnings`)
- [ ] Security audit passed (`cargo audit`)
- [ ] Code reviewed and approved
- [ ] Release binary built (`cargo build --release`)

### Configuration
- [ ] Production config file created (`config/production.toml`)
- [ ] Environment variables documented
- [ ] Secrets stored securely (not in config files)
- [ ] Database connection string configured
- [ ] Redis connection string configured (if using)
- [ ] SMTP/email settings configured
- [ ] OAuth2 credentials configured (if using)
- [ ] Logging level set to `info` or `warn`

### Database
- [ ] Production database created
- [ ] Database user created with appropriate permissions
- [ ] Database migrations tested on staging
- [ ] Database backup plan in place
- [ ] Connection pool sized appropriately (10-20 connections)
- [ ] Indexes created for common queries

### Security
- [ ] HTTPS enabled with valid certificate
- [ ] Session cookies set to `secure = true` and `http_only = true`
- [ ] CSRF protection enabled
- [ ] Security headers configured (HSTS, CSP, X-Frame-Options, etc.)
- [ ] Rate limiting enabled on auth endpoints
- [ ] Input validation on all user inputs
- [ ] SQL injection prevention verified (parameterized queries)
- [ ] Passwords hashed with Argon2id
- [ ] Secrets in environment variables (not hardcoded)
- [ ] File upload validation enabled (if using uploads)

### Infrastructure
- [ ] Reverse proxy configured (Nginx/Caddy)
- [ ] SSL/TLS certificate obtained and installed
- [ ] Firewall rules configured
- [ ] Static files served via reverse proxy or CDN
- [ ] Health check endpoint exposed (`/health`)
- [ ] Metrics endpoint exposed (`/metrics`) with access restrictions
- [ ] Log rotation configured
- [ ] Systemd service file created
- [ ] Service set to start on boot

### Monitoring
- [ ] Prometheus scraping configured
- [ ] Grafana dashboards imported
- [ ] Alert rules configured
- [ ] Alertmanager configured (email/Slack/PagerDuty)
- [ ] Health checks monitored
- [ ] Error tracking configured (Sentry, etc.)
- [ ] Log aggregation set up
- [ ] Uptime monitoring configured

### Performance
- [ ] Database queries optimized
- [ ] N+1 query problems resolved
- [ ] Gzip/Brotli compression enabled
- [ ] Static assets served with cache headers
- [ ] CDN configured (if needed)
- [ ] Connection pooling configured
- [ ] Redis caching enabled for sessions (if distributed)

### Documentation
- [ ] Deployment runbook created
- [ ] Environment variables documented
- [ ] Rollback procedure documented
- [ ] Emergency contacts documented
- [ ] On-call rotation established (if applicable)

## Deployment Steps

### 1. Pre-Deployment Validation
- [ ] Backup production database
- [ ] Notify team of deployment
- [ ] Set up deployment window
- [ ] Test deployment on staging
- [ ] Verify all checklist items above

### 2. Deployment
- [ ] Upload new binary to server
- [ ] Run database migrations
- [ ] Update configuration if needed
- [ ] Restart service
- [ ] Verify health checks pass
- [ ] Monitor logs for errors
- [ ] Check metrics in Grafana

### 3. Post-Deployment Validation
- [ ] Health check endpoint returns 200
- [ ] Application responds to requests
- [ ] Authentication works
- [ ] Database connectivity verified
- [ ] Background jobs processing
- [ ] No errors in logs
- [ ] Metrics showing in Prometheus
- [ ] SSL certificate valid
- [ ] Static files loading

### 4. Smoke Tests
- [ ] Homepage loads
- [ ] Login works
- [ ] Registration works
- [ ] Main features functional
- [ ] Forms submit correctly
- [ ] File uploads work (if applicable)
- [ ] Email sending works (if applicable)

## Post-Deployment

### Immediate (First Hour)
- [ ] Monitor error rates
- [ ] Check response times
- [ ] Verify no 500 errors
- [ ] Check database connections
- [ ] Monitor memory usage
- [ ] Monitor CPU usage
- [ ] Verify logs are being written

### Short-Term (First Day)
- [ ] Monitor for errors
- [ ] Check for memory leaks
- [ ] Verify background jobs running
- [ ] Check database performance
- [ ] Monitor alert notifications
- [ ] Review user feedback

### Long-Term (First Week)
- [ ] Analyze performance metrics
- [ ] Review error logs
- [ ] Optimize slow queries
- [ ] Adjust resource limits if needed
- [ ] Update documentation with lessons learned

## Rollback Plan

### When to Rollback
- [ ] High error rate (> 5% of requests)
- [ ] Application crash loop
- [ ] Database corruption
- [ ] Critical functionality broken
- [ ] Security vulnerability discovered

### Rollback Steps
1. [ ] Stop new service
2. [ ] Restore previous binary
3. [ ] Revert database migrations (if safe)
4. [ ] Restart service
5. [ ] Verify health checks pass
6. [ ] Notify team of rollback
7. [ ] Investigate root cause

### Database Rollback
- [ ] Restore from backup (if needed)
- [ ] Revert migrations (if safe)
- [ ] Verify data integrity
- [ ] Check application compatibility

## Security Hardening

### Application
- [ ] Remove debug endpoints in production
- [ ] Disable detailed error messages for users
- [ ] Validate all user inputs
- [ ] Sanitize HTML output
- [ ] Use prepared statements for SQL
- [ ] Implement request rate limiting
- [ ] Add CAPTCHA to public forms
- [ ] Enable audit logging

### System
- [ ] Run application as non-root user
- [ ] Use systemd security features (NoNewPrivileges, PrivateTmp, etc.)
- [ ] Set resource limits (memory, CPU, file descriptors)
- [ ] Enable SELinux/AppArmor (if applicable)
- [ ] Restrict file permissions (chmod 600 for secrets)
- [ ] Disable unnecessary services
- [ ] Keep system packages updated

### Network
- [ ] Configure firewall (allow only necessary ports)
- [ ] Use private network for database
- [ ] Enable DDoS protection (Cloudflare, etc.)
- [ ] Restrict admin access by IP
- [ ] Use VPN for administrative access

## Performance Tuning

### Application
- [ ] Set appropriate worker count (num_cpus)
- [ ] Configure connection pool size (10-20)
- [ ] Enable caching where appropriate
- [ ] Optimize database queries
- [ ] Use database indexes
- [ ] Implement pagination for large lists

### System
- [ ] Increase file descriptor limit (LimitNOFILE=65536)
- [ ] Configure kernel parameters for high traffic
- [ ] Set appropriate ulimits
- [ ] Enable swap if needed
- [ ] Monitor disk I/O

### Database
- [ ] Configure shared_buffers (25% of RAM)
- [ ] Set effective_cache_size (50-75% of RAM)
- [ ] Enable query logging for slow queries
- [ ] Run VACUUM regularly
- [ ] Configure autovacuum appropriately

## Compliance & Legal

### GDPR (if applicable)
- [ ] Privacy policy published
- [ ] Cookie consent implemented
- [ ] Data deletion process in place
- [ ] Data export functionality available
- [ ] Data processing documented

### Accessibility
- [ ] WCAG 2.1 Level AA compliance verified
- [ ] Screen reader tested
- [ ] Keyboard navigation working
- [ ] Color contrast sufficient

## Final Sign-Off

- [ ] Team lead approval
- [ ] Security review completed
- [ ] Performance benchmarks met
- [ ] Disaster recovery plan tested
- [ ] Runbook reviewed
- [ ] All stakeholders notified

---

**Deployment Date**: _______________

**Deployed By**: _______________

**Verified By**: _______________

**Notes**:
_________________________________________________________________
_________________________________________________________________
_________________________________________________________________

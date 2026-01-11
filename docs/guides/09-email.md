# Email System Guide

**Status**: Phase 2, Week 10 ✅
**Last Updated**: 2025-11-21

---

## Overview

acton-htmx provides a flexible, production-ready email system with:

- **Multiple Backends**: SMTP, AWS SES, Console (development)
- **Template Integration**: Askama templates for HTML and plain text emails
- **Background Jobs**: Async email sending via job queue
- **Testing Utilities**: Mock email sender for tests
- **Type Safety**: Fluent API with compile-time validation

---

## Quick Start

### 1. Basic Email

```rust
use acton_htmx::prelude::*;

async fn send_welcome_email() -> Result<(), EmailError> {
    // Create backend (SMTP example)
    let backend = SmtpBackend::from_env()?;

    // Build email
    let email = Email::new()
        .to("user@example.com")
        .from("noreply@myapp.com")
        .subject("Welcome!")
        .text("Welcome to our app!")
        .html("<h1>Welcome to our app!</h1>");

    // Send email
    backend.send(email).await?;

    Ok(())
}
```

### 2. Email Templates

Create Askama templates for professional emails:

**templates/emails/welcome.html**:
```html
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: Arial, sans-serif; }
        .button { background-color: #4CAF50; color: white; }
    </style>
</head>
<body>
    <h1>Welcome, {{ name }}!</h1>
    <p>Click below to verify your email:</p>
    <a href="{{ verification_url }}" class="button">Verify Email</a>
</body>
</html>
```

**templates/emails/welcome.txt**:
```text
Welcome, {{ name }}!

To verify your email, visit:
{{ verification_url }}

Best regards,
The Team
```

**Rust code**:
```rust
use acton_htmx::prelude::*;
use askama::Template;

#[derive(Template)]
#[template(path = "emails/welcome.html")]
struct WelcomeEmail {
    name: String,
    verification_url: String,
}

impl SimpleEmailTemplate for WelcomeEmail {}

async fn send_templated_email() -> Result<(), EmailError> {
    let backend = SmtpBackend::from_env()?;

    let template = WelcomeEmail {
        name: "Alice".to_string(),
        verification_url: "https://app.com/verify/abc123".to_string(),
    };

    let email = Email::from_template(&template)?
        .to("alice@example.com")
        .from("noreply@myapp.com")
        .subject("Welcome to Our App!");

    backend.send(email).await?;

    Ok(())
}
```

### 3. Background Email Jobs

Send emails asynchronously without blocking HTTP requests:

```rust
use acton_htmx::prelude::*;

async fn register_handler(
    State(state): State<ActonHtmxState>,
    Form(form): Form<RegisterForm>,
) -> Result<Response> {
    // Create user
    let user = create_user(&state.db, form).await?;

    // Queue welcome email (non-blocking)
    let email = Email::new()
        .to(&user.email)
        .from("noreply@myapp.com")
        .subject("Welcome!")
        .text("Welcome to our app!");

    let job = SendEmailJob::new(email);
    state.jobs.enqueue(job).await?;

    // Respond immediately
    Ok(HxRedirect::to("/dashboard").into_response())
}
```

---

## Email Backends

### SMTP (Production)

Best for: General production use with any SMTP server.

**Environment Variables**:
```bash
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=your-email@gmail.com
SMTP_PASSWORD=your-app-password
SMTP_USE_TLS=true
```

**Code**:
```rust
use acton_htmx::prelude::*;

async fn example() -> Result<(), EmailError> {
    // Create backend from environment variables
    let backend = SmtpBackend::from_env()?;

    // Or configure manually
    let config = SmtpConfig {
        host: "smtp.gmail.com".to_string(),
        port: 587,
        username: "your-email@gmail.com".to_string(),
        password: "your-app-password".to_string(),
        use_tls: true,
    };
    let backend = SmtpBackend::new(config);

    Ok(())
}
```

### AWS SES (AWS Environments)

Best for: AWS deployments with high volume.

**Requirements**:
- Enable feature: `aws-sdk-sesv2`
- AWS credentials configured (env vars, IAM role, or ~/.aws/credentials)

**Code**:
```rust
use acton_htmx::prelude::*;

#[cfg(feature = "aws-sdk-sesv2")]
async fn example() -> Result<(), EmailError> {
    // Create backend (uses AWS SDK default credential chain)
    let backend = AwsSesBackend::from_env().await?;

    let email = Email::new()
        .to("user@example.com")
        .from("noreply@myapp.com") // Must be verified in SES
        .subject("Hello from AWS SES")
        .text("Hello!");

    backend.send(email).await?;

    Ok(())
}
```

### Console (Development)

Best for: Development and testing without SMTP credentials.

```rust
use acton_htmx::prelude::*;

async fn example() -> Result<(), EmailError> {
    // Prints emails to console
    let backend = ConsoleBackend::new();

    // Or use verbose mode for full email content
    let backend = ConsoleBackend::verbose();

    let email = Email::new()
        .to("test@example.com")
        .from("dev@localhost")
        .subject("Test Email")
        .text("This will print to console");

    backend.send(email).await?;

    Ok(())
}
```

---

## Email Builder API

### Recipients

```rust
let email = Email::new()
    .to("user1@example.com")
    .to("user2@example.com")               // Multiple recipients
    .to_multiple(&["user3@example.com", "user4@example.com"])
    .cc("manager@example.com")             // Carbon copy
    .bcc("admin@example.com");             // Blind carbon copy
```

### Sender and Reply-To

```rust
let email = Email::new()
    .from("noreply@myapp.com")             // Sender
    .reply_to("support@myapp.com");        // Reply address
```

### Content

```rust
let email = Email::new()
    .subject("Email Subject")
    .text("Plain text content")            // Plain text version
    .html("<h1>HTML content</h1>");        // HTML version
```

**Best Practice**: Always provide both HTML and plain text versions for better compatibility.

### Custom Headers

**⚠️ Current Limitation**: Custom email headers (e.g., `X-Priority`, `X-Campaign-ID`) are not currently supported in the SMTP backend. This is a planned Phase 3 enhancement.

**What Works**:
- Standard headers: `From`, `To`, `CC`, `BCC`, `Reply-To`, `Subject`
- Content-Type headers (automatically set for HTML/text multipart emails)

**What Doesn't Work**:
```rust
let email = Email::new()
    .header("X-Priority", "1")             // ❌ Not implemented
    .header("X-Campaign-ID", "welcome-2025"); // ❌ Not implemented
```

**Workarounds**:

1. **For Priority Emails**: Use a separate SMTP backend with configured priority:
   ```rust
   // Create a dedicated high-priority SMTP connection
   // Configure your SMTP server to prioritize emails from this connection
   let priority_backend = SmtpBackend::new(SmtpConfig {
       host: "smtp-priority.example.com".to_string(),
       port: 587,
       // ... other config
   });
   ```

2. **For Campaign Tracking**: Include tracking data in the email body or use URL parameters:
   ```rust
   let tracking_url = format!(
       "https://myapp.com/track?campaign=welcome-2025&user={}",
       user_id
   );

   let email = Email::new()
       .subject("Welcome!")
       .html(&format!(
           r#"<a href="{}">Click here</a>"#,
           tracking_url
       ));
   ```

3. **For Custom Metadata**: Store metadata in your database associated with sent emails:
   ```rust
   // Store email metadata separately
   sqlx::query!(
       "INSERT INTO email_logs (email_id, campaign_id, priority) VALUES ($1, $2, $3)",
       email_id,
       "welcome-2025",
       1
   ).execute(&pool).await?;
   ```

**Future Enhancement**: Custom header support is tracked for Phase 3. If you need this feature, please open a GitHub issue to help prioritize it.

---

## Email Templates

### Simple Template

For simple emails with one template:

```rust
use acton_htmx::prelude::*;
use askama::Template;

#[derive(Template)]
#[template(path = "emails/notification.html")]
struct NotificationEmail {
    title: String,
    message: String,
}

impl SimpleEmailTemplate for NotificationEmail {}

// Usage
let template = NotificationEmail {
    title: "New Message".to_string(),
    message: "You have a new notification.".to_string(),
};

let email = Email::from_template(&template)?;
```

### HTML + Text Template

For professional emails with separate HTML and text versions:

```rust
use acton_htmx::prelude::*;
use askama::Template;

#[derive(Template)]
#[template(path = "emails/welcome.html")]
struct WelcomeEmailHtml {
    name: String,
    verification_url: String,
}

#[derive(Template)]
#[template(path = "emails/welcome.txt")]
struct WelcomeEmailText {
    name: String,
    verification_url: String,
}

impl EmailTemplate for WelcomeEmailHtml {
    fn render_email(&self) -> Result<(Option<String>, Option<String>), EmailError> {
        let html = self.render()?;
        let text_template = WelcomeEmailText {
            name: self.name.clone(),
            verification_url: self.verification_url.clone(),
        };
        let text = text_template.render()?;
        Ok((Some(html), Some(text)))
    }
}
```

### Template Best Practices

1. **Use Inline CSS**: Email clients have limited CSS support
   ```html
   <p style="color: #333; font-size: 16px;">Text</p>
   ```

2. **Keep Width Under 600px**: For mobile compatibility
   ```html
   <div style="max-width: 600px; margin: 0 auto;">
   ```

3. **Test Across Clients**: Use tools like Litmus or Email on Acid

4. **Plain Text Fallback**: Always provide plain text version

---

## Common Email Flows

### Welcome Email with Verification

```rust
async fn send_verification_email(
    user_email: &str,
    verification_token: &str,
) -> Result<(), EmailError> {
    let backend = SmtpBackend::from_env()?;

    let verification_url = format!(
        "https://myapp.com/verify?token={}",
        verification_token
    );

    let email = Email::new()
        .to(user_email)
        .from("noreply@myapp.com")
        .subject("Verify your email address")
        .text(&format!(
            "Welcome! Please verify your email by visiting: {}",
            verification_url
        ))
        .html(&format!(
            r#"<h1>Welcome!</h1>
            <p>Please verify your email address:</p>
            <a href="{}">Verify Email</a>"#,
            verification_url
        ));

    backend.send(email).await?;

    Ok(())
}
```

### Password Reset

```rust
async fn send_password_reset(
    user_email: &str,
    reset_token: &str,
) -> Result<(), EmailError> {
    let backend = SmtpBackend::from_env()?;

    let reset_url = format!("https://myapp.com/reset?token={}", reset_token);

    let email = Email::new()
        .to(user_email)
        .from("noreply@myapp.com")
        .subject("Password Reset Request")
        .text(&format!(
            "You requested a password reset. Visit: {}",
            reset_url
        ))
        .html(&format!(
            r#"<h1>Password Reset</h1>
            <p>Click below to reset your password:</p>
            <a href="{}">Reset Password</a>
            <p>This link expires in 1 hour.</p>"#,
            reset_url
        ));

    backend.send(email).await?;

    Ok(())
}
```

### Password Changed Notification

```rust
async fn send_password_changed_notification(
    user_email: &str,
) -> Result<(), EmailError> {
    let backend = SmtpBackend::from_env()?;

    let email = Email::new()
        .to(user_email)
        .from("noreply@myapp.com")
        .subject("Your password was changed")
        .text("Your password was successfully changed.")
        .html(
            r#"<h1>Password Changed</h1>
            <p>Your password was successfully changed.</p>
            <p>If you didn't make this change, contact support immediately.</p>"#
        );

    backend.send(email).await?;

    Ok(())
}
```

---

## Testing

### Mock Email Sender

Use the mock sender to test email sending without actually sending emails:

```rust
use acton_htmx::email::MockEmailSender;

#[tokio::test]
async fn test_user_registration_sends_email() {
    let mock_email = MockEmailSender::new();

    // Use mock sender in your handler
    // ...

    // Assertions
    assert_eq!(mock_email.sent_count(), 1);
    assert!(mock_email.was_sent_to("user@example.com"));
    assert!(mock_email.was_sent_with_subject("Welcome!"));

    let sent = mock_email.last_sent().unwrap();
    assert_eq!(sent.from, Some("noreply@myapp.com".to_string()));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_email_validation() {
    let email = Email::new()
        .to("user@example.com")
        .from("noreply@myapp.com")
        .subject("Test")
        .text("Hello");

    // Should validate successfully
    assert!(email.validate().is_ok());
}

#[tokio::test]
async fn test_email_missing_recipient() {
    let email = Email::new()
        .from("noreply@myapp.com")
        .subject("Test")
        .text("Hello");

    // Should fail validation
    assert!(matches!(email.validate(), Err(EmailError::NoRecipients)));
}
```

---

## Security Best Practices

### 1. Prevent Email Injection

Always validate email addresses:

```rust
use validator::Validate;

#[derive(Validate)]
struct EmailForm {
    #[validate(email)]
    email: String,
}

async fn handler(Form(form): Form<EmailForm>) -> Result<Response> {
    form.validate()?; // Validate before using

    let email = Email::new()
        .to(&form.email) // Safe to use after validation
        .from("noreply@myapp.com")
        .subject("Test")
        .text("Hello");

    Ok(Response::default())
}
```

### 2. Rate Limiting

Prevent email bombing:

```rust
use acton_htmx::middleware::RateLimitLayer;

let app = Router::new()
    .route("/send-email", post(send_email_handler))
    .layer(RateLimitLayer::new(
        5,  // 5 requests
        std::time::Duration::from_secs(60), // per minute
    ));
```

### 3. Sanitize User Content

Never trust user input in emails:

```rust
use ammonia::clean;

let user_message = clean(&form.message); // Sanitize HTML

let email = Email::new()
    .html(&format!("<p>{}</p>", user_message));
```

### 4. Use TLS

Always use TLS for SMTP:

```rust
let config = SmtpConfig {
    host: "smtp.example.com".to_string(),
    port: 587,
    username: "user@example.com".to_string(),
    password: "password".to_string(),
    use_tls: true, // Always enable TLS
};
```

---

## Production Configuration

### Environment Variables

Create a `.env` file for development:

```bash
# SMTP Configuration
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=your-app@gmail.com
SMTP_PASSWORD=your-app-specific-password
SMTP_USE_TLS=true

# Application
EMAIL_FROM=noreply@myapp.com
EMAIL_REPLY_TO=support@myapp.com
```

### Deployment Checklist

- [ ] Use environment variables for credentials (never commit passwords)
- [ ] Enable TLS/SSL for SMTP connections
- [ ] Verify sender domain (SPF, DKIM, DMARC records)
- [ ] Set up email rate limiting
- [ ] Monitor bounce and complaint rates
- [ ] Test with real email clients (Gmail, Outlook, etc.)
- [ ] Use background jobs for non-critical emails
- [ ] Implement retry logic with exponential backoff
- [ ] Log email sending failures for debugging

---

## Performance Tips

### 1. Use Background Jobs

Don't block HTTP requests:

```rust
// ❌ Bad: Blocks HTTP request
async fn handler() -> Result<Response> {
    backend.send(email).await?; // Blocks for ~1 second
    Ok(Response::default())
}

// ✅ Good: Queue email job
async fn handler(State(state): State<AppState>) -> Result<Response> {
    state.jobs.enqueue(SendEmailJob::new(email)).await?;
    Ok(Response::default()) // Returns immediately
}
```

### 2. Batch Emails

For bulk sending:

```rust
let emails = vec![/* ... */];

// Send in batches
for chunk in emails.chunks(100) {
    backend.send_batch(chunk.to_vec()).await?;
    tokio::time::sleep(Duration::from_secs(1)).await; // Rate limiting
}
```

### 3. Connection Pooling

Reuse SMTP connections:

```rust
// Create backend once at startup
let backend = Arc::new(SmtpBackend::from_env()?);

// Reuse in handlers
async fn handler(
    State(backend): State<Arc<SmtpBackend>>,
) -> Result<Response> {
    backend.send(email).await?;
    Ok(Response::default())
}
```

---

## Troubleshooting

### SMTP Authentication Failed

**Error**: "Authentication failed" or "Invalid credentials"

**Solutions**:
1. Enable "Less secure app access" (Gmail)
2. Use app-specific passwords (Gmail, Outlook)
3. Verify username and password are correct
4. Check SMTP host and port

### Connection Timeout

**Error**: "Connection timeout" or "Network unreachable"

**Solutions**:
1. Verify firewall allows outbound connections on SMTP port
2. Check SMTP host is reachable (`telnet smtp.gmail.com 587`)
3. Verify TLS settings match server requirements

### Email Goes to Spam

**Solutions**:
1. Set up SPF, DKIM, and DMARC records
2. Verify sender domain
3. Use reputable SMTP service (AWS SES, SendGrid)
4. Avoid spam trigger words in subject/body
5. Include plain text version
6. Add unsubscribe link (for bulk emails)

---

## See Also

- [Background Jobs](07-background-jobs.md) - Job queue integration
- [Authentication](03-authentication.md) - Email verification flows
- [Forms](04-forms.md) - Email validation
- [Askama Documentation](https://djc.github.io/askama/) - Template syntax

---

**Next**: [OAuth2 Integration](10-oauth2.md) →

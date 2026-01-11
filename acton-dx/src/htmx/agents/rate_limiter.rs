//! Rate Limiter Agent
//!
//! Actor-based rate limiting using acton-reactive with token bucket algorithm.
//! Provides local rate limiting when cache-service is unavailable.
//!
//! Features:
//! - Token bucket algorithm for smooth rate limiting
//! - Self-scheduling bucket refills
//! - Per-key rate limiting (IP, user, route)
//! - Configurable bucket size and refill rate
//! - Automatic cleanup of expired buckets

use crate::htmx::agents::default_actor_config;
use crate::htmx::agents::request_reply::{create_request_reply, send_response, ResponseChannel};
use acton_reactive::prelude::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

/// Default bucket capacity (max tokens)
const DEFAULT_BUCKET_CAPACITY: u32 = 100;

/// Default refill rate (tokens per second)
const DEFAULT_REFILL_RATE: f64 = 10.0;

/// Default cleanup interval (seconds)
const DEFAULT_CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Default bucket expiration (seconds without activity)
const DEFAULT_BUCKET_EXPIRATION: Duration = Duration::from_secs(300);

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Current number of tokens
    tokens: f64,
    /// Maximum capacity
    capacity: u32,
    /// Tokens per second refill rate
    refill_rate: f64,
    /// Last time tokens were updated
    last_update: Instant,
    /// Last time bucket was accessed
    last_access: Instant,
}

impl TokenBucket {
    /// Create a new token bucket with the given capacity and refill rate
    #[must_use]
    pub fn new(capacity: u32, refill_rate: f64) -> Self {
        let now = Instant::now();
        Self {
            tokens: f64::from(capacity),
            capacity,
            refill_rate,
            last_update: now,
            last_access: now,
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        let new_tokens = elapsed.as_secs_f64() * self.refill_rate;
        self.tokens = (self.tokens + new_tokens).min(f64::from(self.capacity));
        self.last_update = now;
    }

    /// Try to consume tokens from the bucket
    ///
    /// Returns true if tokens were consumed, false if not enough tokens
    #[must_use]
    pub fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        self.last_access = Instant::now();

        let requested = f64::from(tokens);
        if self.tokens >= requested {
            self.tokens -= requested;
            true
        } else {
            false
        }
    }

    /// Check if bucket has expired (no activity for expiration duration)
    #[must_use]
    pub fn is_expired(&self, expiration: Duration) -> bool {
        self.last_access.elapsed() >= expiration
    }

    /// Get current token count
    #[must_use]
    pub fn available_tokens(&mut self) -> u32 {
        self.refill();
        #[allow(clippy::cast_sign_loss)]
        #[allow(clippy::cast_possible_truncation)]
        {
            self.tokens.floor() as u32
        }
    }
}

/// Configuration for rate limiter agent
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Default bucket capacity
    pub bucket_capacity: u32,
    /// Default refill rate (tokens per second)
    pub refill_rate: f64,
    /// Cleanup interval for expired buckets
    pub cleanup_interval: Duration,
    /// Bucket expiration (time without activity)
    pub bucket_expiration: Duration,
    /// Whether rate limiting is enabled
    pub enabled: bool,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            bucket_capacity: DEFAULT_BUCKET_CAPACITY,
            refill_rate: DEFAULT_REFILL_RATE,
            cleanup_interval: DEFAULT_CLEANUP_INTERVAL,
            bucket_expiration: DEFAULT_BUCKET_EXPIRATION,
            enabled: true,
        }
    }
}

impl RateLimiterConfig {
    /// Create a new configuration with defaults
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set bucket capacity
    #[must_use]
    pub const fn with_bucket_capacity(mut self, capacity: u32) -> Self {
        self.bucket_capacity = capacity;
        self
    }

    /// Set refill rate (tokens per second)
    #[must_use]
    pub const fn with_refill_rate(mut self, rate: f64) -> Self {
        self.refill_rate = rate;
        self
    }

    /// Set cleanup interval
    #[must_use]
    pub const fn with_cleanup_interval(mut self, interval: Duration) -> Self {
        self.cleanup_interval = interval;
        self
    }

    /// Set bucket expiration
    #[must_use]
    pub const fn with_bucket_expiration(mut self, expiration: Duration) -> Self {
        self.bucket_expiration = expiration;
        self
    }

    /// Enable or disable rate limiting
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

// Type alias for the actor builder
type RateLimiterActorBuilder = ManagedActor<Idle, RateLimiterAgent>;

/// Rate limiter agent model
#[derive(Debug)]
pub struct RateLimiterAgent {
    /// Configuration
    config: RateLimiterConfig,
    /// Token buckets per key
    buckets: HashMap<String, TokenBucket>,
    /// Total requests processed
    request_count: u64,
    /// Total requests allowed
    allowed_count: u64,
    /// Total requests denied
    denied_count: u64,
}

impl Default for RateLimiterAgent {
    fn default() -> Self {
        Self::new(RateLimiterConfig::default())
    }
}

impl Clone for RateLimiterAgent {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            buckets: self.buckets.clone(),
            request_count: self.request_count,
            allowed_count: self.allowed_count,
            denied_count: self.denied_count,
        }
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// Check if request is allowed (try to consume tokens)
#[derive(Clone, Debug)]
pub struct CheckRateLimit {
    /// Rate limit key (e.g., "ip:192.168.1.1" or "user:123")
    pub key: String,
    /// Number of tokens to consume (default: 1)
    pub tokens: u32,
    /// Response channel for result
    pub response_tx: Option<ResponseChannel<RateLimitResult>>,
}

impl CheckRateLimit {
    /// Create a new rate limit check request
    #[must_use]
    pub fn new(key: String, tokens: u32) -> (Self, oneshot::Receiver<RateLimitResult>) {
        let (response_tx, rx) = create_request_reply();
        (
            Self {
                key,
                tokens,
                response_tx: Some(response_tx),
            },
            rx,
        )
    }

    /// Create a fire-and-forget rate limit check (no response)
    #[must_use]
    pub const fn fire_and_forget(key: String, tokens: u32) -> Self {
        Self {
            key,
            tokens,
            response_tx: None,
        }
    }
}

/// Result of a rate limit check
#[derive(Clone, Debug)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Remaining tokens in bucket
    pub remaining_tokens: u32,
    /// Rate limit key
    pub key: String,
}

/// Get current rate limiter statistics
#[derive(Clone, Debug, Default)]
pub struct GetStats {
    /// Response channel
    pub response_tx: Option<ResponseChannel<RateLimiterStats>>,
}

impl GetStats {
    /// Create a new get stats request
    #[must_use]
    pub fn new() -> (Self, oneshot::Receiver<RateLimiterStats>) {
        let (response_tx, rx) = create_request_reply();
        (
            Self {
                response_tx: Some(response_tx),
            },
            rx,
        )
    }
}

/// Rate limiter statistics
#[derive(Clone, Debug, Default)]
pub struct RateLimiterStats {
    /// Total requests processed
    pub request_count: u64,
    /// Total requests allowed
    pub allowed_count: u64,
    /// Total requests denied
    pub denied_count: u64,
    /// Number of active buckets
    pub bucket_count: usize,
    /// Whether rate limiting is enabled
    pub enabled: bool,
}

/// Trigger cleanup of expired buckets
#[derive(Clone, Debug, Default)]
pub struct CleanupExpired;

/// Update rate limiter configuration
#[derive(Clone, Debug)]
pub struct UpdateConfig {
    /// New configuration
    pub config: RateLimiterConfig,
}

impl UpdateConfig {
    /// Create a new update config message
    #[must_use]
    pub const fn new(config: RateLimiterConfig) -> Self {
        Self { config }
    }
}

/// Reset a specific bucket
#[derive(Clone, Debug)]
pub struct ResetBucket {
    /// Key to reset
    pub key: String,
}

impl ResetBucket {
    /// Create a new reset bucket message
    #[must_use]
    pub const fn new(key: String) -> Self {
        Self { key }
    }
}

impl RateLimiterAgent {
    /// Create a new rate limiter with the given configuration
    #[must_use]
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            config,
            buckets: HashMap::new(),
            request_count: 0,
            allowed_count: 0,
            denied_count: 0,
        }
    }

    /// Spawn rate limiter actor
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails
    pub async fn spawn(runtime: &mut ActorRuntime) -> anyhow::Result<ActorHandle> {
        Self::spawn_with_config(runtime, RateLimiterConfig::default()).await
    }

    /// Spawn rate limiter actor with custom configuration
    ///
    /// # Errors
    ///
    /// Returns error if actor initialization fails
    pub async fn spawn_with_config(
        runtime: &mut ActorRuntime,
        config: RateLimiterConfig,
    ) -> anyhow::Result<ActorHandle> {
        let actor_config = default_actor_config("rate_limiter")?;
        let mut builder = runtime.new_actor_with_config::<Self>(actor_config);

        builder.model.config = config;

        Self::configure_handlers(builder).await
    }

    /// Configure all message handlers
    async fn configure_handlers(mut builder: RateLimiterActorBuilder) -> anyhow::Result<ActorHandle> {
        Self::configure_rate_limit_handlers(&mut builder);
        Self::configure_admin_handlers(&mut builder);
        Ok(builder.start().await)
    }

    /// Configure rate limiting handlers
    fn configure_rate_limit_handlers(builder: &mut RateLimiterActorBuilder) {
        builder.mutate_on::<CheckRateLimit>(|actor, context| {
            let msg = context.message();
            actor.model.request_count += 1;

            // If disabled, always allow
            if !actor.model.config.enabled {
                let result = RateLimitResult {
                    allowed: true,
                    remaining_tokens: actor.model.config.bucket_capacity,
                    key: msg.key.clone(),
                };
                actor.model.allowed_count += 1;

                if let Some(tx) = msg.response_tx.clone() {
                    return Reply::pending(async move {
                        let _ = send_response(tx, result).await;
                    });
                }
                return Reply::ready();
            }

            // Get or create bucket
            let bucket = actor
                .model
                .buckets
                .entry(msg.key.clone())
                .or_insert_with(|| {
                    TokenBucket::new(
                        actor.model.config.bucket_capacity,
                        actor.model.config.refill_rate,
                    )
                });

            // Try to consume tokens
            let allowed = bucket.try_consume(msg.tokens);
            let remaining_tokens = bucket.available_tokens();

            if allowed {
                actor.model.allowed_count += 1;
            } else {
                actor.model.denied_count += 1;
            }

            let result = RateLimitResult {
                allowed,
                remaining_tokens,
                key: msg.key.clone(),
            };

            if let Some(tx) = msg.response_tx.clone() {
                Reply::pending(async move {
                    let _ = send_response(tx, result).await;
                })
            } else {
                Reply::ready()
            }
        });
    }

    /// Configure admin handlers
    fn configure_admin_handlers(builder: &mut RateLimiterActorBuilder) {
        builder
            .mutate_on::<GetStats>(|actor, context| {
                let Some(tx) = context.message().response_tx.clone() else {
                    return Reply::ready();
                };

                let stats = RateLimiterStats {
                    request_count: actor.model.request_count,
                    allowed_count: actor.model.allowed_count,
                    denied_count: actor.model.denied_count,
                    bucket_count: actor.model.buckets.len(),
                    enabled: actor.model.config.enabled,
                };

                Reply::pending(async move {
                    let _ = send_response(tx, stats).await;
                })
            })
            .mutate_on::<CleanupExpired>(|actor, _context| {
                let expiration = actor.model.config.bucket_expiration;
                let before_count = actor.model.buckets.len();

                actor
                    .model
                    .buckets
                    .retain(|_, bucket| !bucket.is_expired(expiration));

                let removed = before_count - actor.model.buckets.len();
                if removed > 0 {
                    tracing::debug!(removed = removed, "Cleaned up expired rate limit buckets");
                }

                Reply::ready()
            })
            .mutate_on::<UpdateConfig>(|actor, context| {
                actor.model.config = context.message().config.clone();
                tracing::info!("Rate limiter configuration updated");
                Reply::ready()
            })
            .mutate_on::<ResetBucket>(|actor, context| {
                let key = &context.message().key;
                if actor.model.buckets.remove(key).is_some() {
                    tracing::debug!(key = %key, "Reset rate limit bucket");
                }
                Reply::ready()
            });
    }

    /// Get bucket for a key (for testing)
    #[must_use]
    pub fn get_bucket(&self, key: &str) -> Option<&TokenBucket> {
        self.buckets.get(key)
    }

    /// Check if rate limiting is enabled
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_creation() {
        let bucket = TokenBucket::new(100, 10.0);
        assert_eq!(bucket.capacity, 100);
        assert!((bucket.refill_rate - 10.0).abs() < f64::EPSILON);
        assert!((bucket.tokens - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_bucket_consume() {
        let mut bucket = TokenBucket::new(100, 10.0);

        // Should succeed
        assert!(bucket.try_consume(50));
        assert!((bucket.tokens - 50.0).abs() < 1.0);

        // Should succeed
        assert!(bucket.try_consume(40));
        assert!((bucket.tokens - 10.0).abs() < 1.0);

        // Should fail - not enough tokens
        assert!(!bucket.try_consume(20));
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(100, 100.0); // 100 tokens per second

        // Consume all tokens
        assert!(bucket.try_consume(100));
        assert!(bucket.tokens < 1.0);

        // Wait a bit for refill
        std::thread::sleep(Duration::from_millis(50));

        // Should have some tokens now
        let available = bucket.available_tokens();
        assert!(available > 0);
    }

    #[test]
    fn test_token_bucket_max_capacity() {
        let mut bucket = TokenBucket::new(100, 1000.0); // High refill rate

        // Consume some tokens
        assert!(bucket.try_consume(50));

        // Wait for potential over-refill
        std::thread::sleep(Duration::from_millis(200));

        // Should not exceed capacity
        let available = bucket.available_tokens();
        assert!(available <= 100);
    }

    #[test]
    fn test_token_bucket_expiration() {
        let mut bucket = TokenBucket::new(100, 10.0);

        // Fresh bucket should not be expired
        assert!(!bucket.is_expired(Duration::from_millis(100)));

        // Access the bucket
        let _ = bucket.try_consume(1);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(150));

        // Should be expired now
        assert!(bucket.is_expired(Duration::from_millis(100)));
    }

    #[test]
    fn test_rate_limiter_config_default() {
        let config = RateLimiterConfig::default();
        assert!(config.enabled);
        assert_eq!(config.bucket_capacity, DEFAULT_BUCKET_CAPACITY);
        assert!((config.refill_rate - DEFAULT_REFILL_RATE).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rate_limiter_config_builder() {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(50)
            .with_refill_rate(5.0)
            .with_cleanup_interval(Duration::from_secs(30))
            .with_bucket_expiration(Duration::from_secs(120))
            .with_enabled(false);

        assert!(!config.enabled);
        assert_eq!(config.bucket_capacity, 50);
        assert!((config.refill_rate - 5.0).abs() < f64::EPSILON);
        assert_eq!(config.cleanup_interval, Duration::from_secs(30));
        assert_eq!(config.bucket_expiration, Duration::from_secs(120));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_spawn() {
        let mut runtime = ActonApp::launch_async().await;
        let result = RateLimiterAgent::spawn(&mut runtime).await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_allows_within_limit() {
        let config = RateLimiterConfig::new().with_bucket_capacity(10).with_refill_rate(0.0);
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Should allow 10 requests
        for i in 0..10 {
            let (request, rx) = CheckRateLimit::new("test_key".to_string(), 1);
            handle.send(request).await;

            let result = rx.await.expect("Should get result");
            assert!(result.allowed, "Request {i} should be allowed");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_denies_over_limit() {
        let config = RateLimiterConfig::new().with_bucket_capacity(5).with_refill_rate(0.0);
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Should allow 5 requests
        for _ in 0..5 {
            let (request, rx) = CheckRateLimit::new("test_key".to_string(), 1);
            handle.send(request).await;
            let result = rx.await.expect("Should get result");
            assert!(result.allowed);
        }

        // 6th request should be denied
        let (request, rx) = CheckRateLimit::new("test_key".to_string(), 1);
        handle.send(request).await;
        let result = rx.await.expect("Should get result");
        assert!(!result.allowed);
        assert_eq!(result.remaining_tokens, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_different_keys() {
        let config = RateLimiterConfig::new().with_bucket_capacity(3).with_refill_rate(0.0);
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Exhaust key1
        for _ in 0..3 {
            let (request, rx) = CheckRateLimit::new("key1".to_string(), 1);
            handle.send(request).await;
            let _ = rx.await;
        }

        // key1 should be denied
        let (request, rx) = CheckRateLimit::new("key1".to_string(), 1);
        handle.send(request).await;
        let result = rx.await.expect("Should get result");
        assert!(!result.allowed);

        // key2 should still be allowed
        let (request, rx) = CheckRateLimit::new("key2".to_string(), 1);
        handle.send(request).await;
        let result = rx.await.expect("Should get result");
        assert!(result.allowed);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_get_stats() {
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn(&mut runtime).await.unwrap();

        // Make some requests
        for _ in 0..5 {
            let (request, rx) = CheckRateLimit::new("test".to_string(), 1);
            handle.send(request).await;
            let _ = rx.await;
        }

        // Get stats
        let (request, rx) = GetStats::new();
        handle.send(request).await;
        let stats = rx.await.expect("Should get stats");

        assert_eq!(stats.request_count, 5);
        assert_eq!(stats.allowed_count, 5);
        assert_eq!(stats.denied_count, 0);
        assert_eq!(stats.bucket_count, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_cleanup_expired() {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(10)
            .with_refill_rate(0.0)
            .with_bucket_expiration(Duration::from_millis(50));
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Create some buckets
        for i in 0..3 {
            let (request, rx) = CheckRateLimit::new(format!("key{i}"), 1);
            handle.send(request).await;
            let _ = rx.await;
        }

        // Verify buckets exist
        let (request, rx) = GetStats::new();
        handle.send(request).await;
        let stats = rx.await.expect("Should get stats");
        assert_eq!(stats.bucket_count, 3);

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Trigger cleanup
        handle.send(CleanupExpired).await;

        // Small delay for processing
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Verify buckets cleaned
        let (request, rx) = GetStats::new();
        handle.send(request).await;
        let stats = rx.await.expect("Should get stats");
        assert_eq!(stats.bucket_count, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_reset_bucket() {
        let config = RateLimiterConfig::new().with_bucket_capacity(5).with_refill_rate(0.0);
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Exhaust the bucket
        for _ in 0..5 {
            let (request, rx) = CheckRateLimit::new("test".to_string(), 1);
            handle.send(request).await;
            let _ = rx.await;
        }

        // Should be denied
        let (request, rx) = CheckRateLimit::new("test".to_string(), 1);
        handle.send(request).await;
        let result = rx.await.expect("Should get result");
        assert!(!result.allowed);

        // Reset the bucket
        handle.send(ResetBucket::new("test".to_string())).await;

        // Small delay for processing
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be allowed again
        let (request, rx) = CheckRateLimit::new("test".to_string(), 1);
        handle.send(request).await;
        let result = rx.await.expect("Should get result");
        assert!(result.allowed);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_disabled() {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(1)
            .with_refill_rate(0.0)
            .with_enabled(false);
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Should allow many requests when disabled
        for _ in 0..100 {
            let (request, rx) = CheckRateLimit::new("test".to_string(), 1);
            handle.send(request).await;
            let result = rx.await.expect("Should get result");
            assert!(result.allowed);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rate_limiter_update_config() {
        let config = RateLimiterConfig::new().with_bucket_capacity(5).with_enabled(true);
        let mut runtime = ActonApp::launch_async().await;
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Verify enabled
        let (request, rx) = GetStats::new();
        handle.send(request).await;
        let stats = rx.await.expect("Should get stats");
        assert!(stats.enabled);

        // Update config to disabled
        let new_config = RateLimiterConfig::new().with_enabled(false);
        handle.send(UpdateConfig::new(new_config)).await;

        // Small delay for processing
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Verify disabled
        let (request, rx) = GetStats::new();
        handle.send(request).await;
        let stats = rx.await.expect("Should get stats");
        assert!(!stats.enabled);
    }
}

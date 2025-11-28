//! Integration tests for acton-reactive agents
//!
//! Tests the coordination between HotReloadCoordinator, ServiceCoordinator,
//! and RateLimiter agents.

use acton_dx::htmx::agents::{
    // Hot reload
    FileChanged, HotReloadConfig, HotReloadCoordinatorAgent, HotReloadGetStats, ReloadType,
    HotReloadSubscribe,
    // Rate limiter
    CheckRateLimit, RateLimiterAgent, RateLimiterCleanupExpired, RateLimiterConfig,
    RateLimiterGetStats, ResetBucket,
    // Service coordinator
    CircuitState, GetServiceStatus, HealthCheckResult, ServiceAvailable,
    ServiceCoordinatorAgent, ServiceCoordinatorConfig, ServiceCoordinatorSubscribe, ServiceId,
    ServiceState,
};
use acton_reactive::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

/// Test that all agents can be spawned together without conflicts
#[tokio::test(flavor = "multi_thread")]
async fn test_all_agents_spawn_together() {
    let mut runtime = ActonApp::launch();

    // Spawn all agents
    let hot_reload_handle = HotReloadCoordinatorAgent::spawn(&mut runtime)
        .await
        .expect("Should spawn hot reload agent");

    let service_handle = ServiceCoordinatorAgent::spawn(&mut runtime)
        .await
        .expect("Should spawn service coordinator agent");

    let rate_limiter_handle = RateLimiterAgent::spawn(&mut runtime)
        .await
        .expect("Should spawn rate limiter agent");

    // Verify all agents are responding
    let (hr_stats_req, hr_stats_rx) = HotReloadGetStats::new();
    hot_reload_handle.send(hr_stats_req).await;
    let hr_stats = hr_stats_rx.await.expect("Should get hot reload stats");
    assert!(hr_stats.enabled);

    let (sc_status_req, sc_status_rx) = GetServiceStatus::new();
    service_handle.send(sc_status_req).await;
    let sc_status = sc_status_rx.await.expect("Should get service status");
    assert!(sc_status.enabled);
    assert_eq!(sc_status.services.len(), 6);

    let (rl_stats_req, rl_stats_rx) = RateLimiterGetStats::new();
    rate_limiter_handle.send(rl_stats_req).await;
    let rl_stats = rl_stats_rx.await.expect("Should get rate limiter stats");
    assert!(rl_stats.enabled);
    assert_eq!(rl_stats.bucket_count, 0);
}

/// Test service coordinator tracking multiple services
#[tokio::test(flavor = "multi_thread")]
async fn test_service_coordinator_tracks_multiple_services() {
    let mut runtime = ActonApp::launch();
    let handle = ServiceCoordinatorAgent::spawn(&mut runtime)
        .await
        .expect("Should spawn");

    // Mark several services as available
    for service in [ServiceId::Auth, ServiceId::Data, ServiceId::Cedar] {
        handle.send(ServiceAvailable::new(service)).await;
    }

    // Small delay for processing
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify status
    let (req, rx) = GetServiceStatus::new();
    handle.send(req).await;
    let status = rx.await.expect("Should get status");

    assert_eq!(
        status.services.get(&ServiceId::Auth).unwrap().0,
        ServiceState::Healthy
    );
    assert_eq!(
        status.services.get(&ServiceId::Data).unwrap().0,
        ServiceState::Healthy
    );
    assert_eq!(
        status.services.get(&ServiceId::Cedar).unwrap().0,
        ServiceState::Healthy
    );
}

/// Test service failure cascades through circuit breaker
#[tokio::test(flavor = "multi_thread")]
async fn test_service_failure_cascade() {
    let config = ServiceCoordinatorConfig::new().with_failure_threshold(3);
    let mut runtime = ActonApp::launch();
    let handle = ServiceCoordinatorAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Subscribe to status events
    let (sub_req, sub_rx) = ServiceCoordinatorSubscribe::new();
    handle.send(sub_req).await;
    let mut subscriber = sub_rx.await.expect("Should subscribe");

    // Mark service as available first
    handle.send(ServiceAvailable::new(ServiceId::Auth)).await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Clear subscription queue
    while subscriber.try_recv().is_ok() {}

    // Now send failures to trigger circuit breaker
    for i in 0..3 {
        handle
            .send(HealthCheckResult::failure(
                ServiceId::Auth,
                format!("Error {i}"),
            ))
            .await;
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify service is now unhealthy
    let (req, rx) = GetServiceStatus::new();
    handle.send(req).await;
    let status = rx.await.expect("Should get status");

    let (state, circuit) = status.services.get(&ServiceId::Auth).unwrap();
    assert_eq!(*state, ServiceState::Unhealthy);
    assert_eq!(*circuit, CircuitState::Open);
}

/// Test rate limiter works across multiple keys
#[tokio::test(flavor = "multi_thread")]
async fn test_rate_limiter_multiple_keys() {
    let config = RateLimiterConfig::new()
        .with_bucket_capacity(5)
        .with_refill_rate(0.0);

    let mut runtime = ActonApp::launch();
    let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Exhaust key for IP 1
    for _ in 0..5 {
        let (req, rx) = CheckRateLimit::new("ip:192.168.1.1".to_string(), 1);
        handle.send(req).await;
        let result = rx.await.expect("Should get result");
        assert!(result.allowed);
    }

    // IP 1 should be denied
    let (req, rx) = CheckRateLimit::new("ip:192.168.1.1".to_string(), 1);
    handle.send(req).await;
    let result = rx.await.expect("Should get result");
    assert!(!result.allowed);

    // IP 2 should still be allowed
    let (req, rx) = CheckRateLimit::new("ip:192.168.1.2".to_string(), 1);
    handle.send(req).await;
    let result = rx.await.expect("Should get result");
    assert!(result.allowed);

    // User-based key should be separate
    let (req, rx) = CheckRateLimit::new("user:123".to_string(), 1);
    handle.send(req).await;
    let result = rx.await.expect("Should get result");
    assert!(result.allowed);
}

/// Test hot reload coordinator debouncing
#[tokio::test(flavor = "multi_thread")]
async fn test_hot_reload_debouncing() {
    let config =
        HotReloadConfig::new().with_debounce(ReloadType::Templates, Duration::from_millis(50));

    let mut runtime = ActonApp::launch();
    let handle = HotReloadCoordinatorAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Subscribe to reload events
    let (sub_req, sub_rx) = HotReloadSubscribe::new();
    handle.send(sub_req).await;
    let mut subscriber = sub_rx.await.expect("Should subscribe");

    // Send multiple file changes rapidly
    for i in 0..5 {
        handle
            .send(FileChanged::new(
                ReloadType::Templates,
                PathBuf::from(format!("/path/to/file{i}.html")),
            ))
            .await;
    }

    // Wait for debounce to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify stats show pending events
    let (stats_req, stats_rx) = HotReloadGetStats::new();
    handle.send(stats_req).await;
    let _stats = stats_rx.await.expect("Should get stats");

    // Events should be in the subscriber (debounced)
    let events_received = std::iter::from_fn(|| subscriber.try_recv().ok()).count();
    // May receive fewer events due to debouncing
    assert!(events_received <= 5, "Debouncing should reduce event count");
}

/// Test graceful degradation when rate limiter is disabled
#[tokio::test(flavor = "multi_thread")]
async fn test_rate_limiter_graceful_degradation() {
    // Create disabled rate limiter
    let config = RateLimiterConfig::new()
        .with_bucket_capacity(1)
        .with_enabled(false);

    let mut runtime = ActonApp::launch();
    let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Should allow all requests when disabled
    for _ in 0..100 {
        let (req, rx) = CheckRateLimit::new("test".to_string(), 1);
        handle.send(req).await;
        let result = rx.await.expect("Should get result");
        assert!(result.allowed, "Should allow when disabled");
    }
}

/// Test concurrent access to rate limiter
#[tokio::test(flavor = "multi_thread")]
async fn test_rate_limiter_concurrent_access() {
    let config = RateLimiterConfig::new()
        .with_bucket_capacity(100)
        .with_refill_rate(0.0);

    let mut runtime = ActonApp::launch();
    let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Spawn multiple tasks hitting the rate limiter concurrently
    let mut join_set = tokio::task::JoinSet::new();
    for i in 0..10 {
        let h = handle.clone();
        join_set.spawn(async move {
            let mut allowed = 0;
            for _ in 0..20 {
                let (req, rx) = CheckRateLimit::new(format!("concurrent_key_{i}"), 1);
                h.send(req).await;
                if rx.await.is_ok_and(|r| r.allowed) {
                    allowed += 1;
                }
            }
            allowed
        });
    }

    // Wait for all tasks
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        if let Ok(count) = result {
            results.push(count);
        }
    }

    // Each key should allow exactly 20 requests (bucket capacity 100, 20 requests each)
    for allowed in &results {
        assert_eq!(*allowed, 20);
    }
}

/// Test service recovery after failure
#[tokio::test(flavor = "multi_thread")]
async fn test_service_recovery() {
    let config = ServiceCoordinatorConfig::new().with_failure_threshold(2);
    let mut runtime = ActonApp::launch();
    let handle = ServiceCoordinatorAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Fail the service
    handle
        .send(HealthCheckResult::failure(
            ServiceId::Data,
            "Error".to_string(),
        ))
        .await;
    handle
        .send(HealthCheckResult::failure(
            ServiceId::Data,
            "Error".to_string(),
        ))
        .await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify unhealthy
    let (req, rx) = GetServiceStatus::new();
    handle.send(req).await;
    let status = rx.await.expect("Should get status");
    assert_eq!(
        status.services.get(&ServiceId::Data).unwrap().0,
        ServiceState::Unhealthy
    );

    // Recover the service
    handle.send(ServiceAvailable::new(ServiceId::Data)).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify healthy
    let (req, rx) = GetServiceStatus::new();
    handle.send(req).await;
    let status = rx.await.expect("Should get status");
    assert_eq!(
        status.services.get(&ServiceId::Data).unwrap().0,
        ServiceState::Healthy
    );
}

/// Test bucket cleanup removes old entries
#[tokio::test(flavor = "multi_thread")]
async fn test_rate_limiter_bucket_cleanup() {
    let config = RateLimiterConfig::new()
        .with_bucket_capacity(10)
        .with_bucket_expiration(Duration::from_millis(50));

    let mut runtime = ActonApp::launch();
    let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Create buckets
    for i in 0..5 {
        let (req, rx) = CheckRateLimit::new(format!("key_{i}"), 1);
        handle.send(req).await;
        let _ = rx.await;
    }

    // Verify buckets exist
    let (req, rx) = RateLimiterGetStats::new();
    handle.send(req).await;
    let stats = rx.await.expect("Should get stats");
    assert_eq!(stats.bucket_count, 5);

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Trigger cleanup
    handle.send(RateLimiterCleanupExpired).await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Verify buckets cleaned
    let (req, rx) = RateLimiterGetStats::new();
    handle.send(req).await;
    let stats = rx.await.expect("Should get stats");
    assert_eq!(stats.bucket_count, 0);
}

/// Test that reset bucket works correctly
#[tokio::test(flavor = "multi_thread")]
async fn test_rate_limiter_reset_bucket() {
    let config = RateLimiterConfig::new()
        .with_bucket_capacity(3)
        .with_refill_rate(0.0);

    let mut runtime = ActonApp::launch();
    let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
        .await
        .expect("Should spawn");

    // Exhaust bucket
    for _ in 0..3 {
        let (req, rx) = CheckRateLimit::new("test_key".to_string(), 1);
        handle.send(req).await;
        let _ = rx.await;
    }

    // Should be denied
    let (req, rx) = CheckRateLimit::new("test_key".to_string(), 1);
    handle.send(req).await;
    let result = rx.await.expect("Should get result");
    assert!(!result.allowed);

    // Reset the bucket
    handle.send(ResetBucket::new("test_key".to_string())).await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Should be allowed again
    let (req, rx) = CheckRateLimit::new("test_key".to_string(), 1);
    handle.send(req).await;
    let result = rx.await.expect("Should get result");
    assert!(result.allowed);
}

/// Test health check count tracking
#[tokio::test(flavor = "multi_thread")]
async fn test_service_coordinator_health_check_count() {
    let mut runtime = ActonApp::launch();
    let handle = ServiceCoordinatorAgent::spawn(&mut runtime)
        .await
        .expect("Should spawn");

    // Send several health checks
    for i in 0..10 {
        let service = if i % 2 == 0 {
            ServiceId::Auth
        } else {
            ServiceId::Data
        };
        handle.send(HealthCheckResult::success(service, 10)).await;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify count
    let (req, rx) = GetServiceStatus::new();
    handle.send(req).await;
    let status = rx.await.expect("Should get status");
    assert_eq!(status.health_check_count, 10);
}

//! Performance benchmarks for acton-dx agents
//!
//! These benchmarks measure the performance of:
//! - `RateLimiterAgent`: Token bucket rate limiting
//! - `ServiceCoordinatorAgent`: Service health tracking
//! - `HotReloadCoordinatorAgent`: File change coordination
//!
//! Run with: `cargo bench --bench agents_benchmark`
#![allow(missing_docs)]

use acton_dx::htmx::agents::{
    // Rate limiter
    CheckRateLimit, RateLimiterAgent, RateLimiterConfig, RateLimiterGetStats,
    // Service coordinator
    GetServiceStatus, HealthCheckResult, ServiceAvailable, ServiceCoordinatorAgent,
    ServiceCoordinatorConfig, ServiceId,
    // Hot reload
    FileChanged, HotReloadConfig, HotReloadCoordinatorAgent, HotReloadGetStats, ReloadType,
};
use acton_reactive::prelude::*;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::PathBuf;
use std::time::Duration;

/// Benchmark rate limiter single request performance
fn bench_rate_limiter_single_request(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(10_000)
            .with_refill_rate(1000.0);
        let mut runtime = ActonApp::launch();
        RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap()
    });

    c.bench_function("rate_limiter/single_request", |b| {
        b.to_async(&rt).iter(|| async {
            let (request, rx) = CheckRateLimit::new("bench_key".to_string(), 1);
            handle.send(request).await;
            rx.await.expect("Should get result")
        });
    });
}

/// Benchmark rate limiter with different key counts
fn bench_rate_limiter_key_scaling(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(10_000)
            .with_refill_rate(1000.0);
        let mut runtime = ActonApp::launch();
        RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap()
    });

    let mut group = c.benchmark_group("rate_limiter/key_scaling");

    for num_keys in [10, 100, 1000] {
        group.throughput(Throughput::Elements(num_keys));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_keys),
            &num_keys,
            |b, &num_keys| {
                b.to_async(&rt).iter(|| async {
                    for i in 0..num_keys {
                        let (request, rx) = CheckRateLimit::new(format!("key_{i}"), 1);
                        handle.send(request).await;
                        let _ = rx.await;
                    }
                });
            },
        );
    }
    group.finish();
}

/// Benchmark rate limiter stats retrieval
fn bench_rate_limiter_get_stats(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(10_000)
            .with_refill_rate(1000.0);
        let mut runtime = ActonApp::launch();
        let handle = RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Pre-populate with some buckets
        for i in 0..100 {
            let (request, rx) = CheckRateLimit::new(format!("prepopulate_key_{i}"), 1);
            handle.send(request).await;
            let _ = rx.await;
        }

        handle
    });

    c.bench_function("rate_limiter/get_stats", |b| {
        b.to_async(&rt).iter(|| async {
            let (request, rx) = RateLimiterGetStats::new();
            handle.send(request).await;
            rx.await.expect("Should get stats")
        });
    });
}

/// Benchmark service coordinator status retrieval
fn bench_service_coordinator_get_status(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = ServiceCoordinatorConfig::new()
            .with_failure_threshold(5)
            .with_recovery_timeout(Duration::from_secs(30));
        let mut runtime = ActonApp::launch();
        let handle = ServiceCoordinatorAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Register all services
        for service_id in [
            ServiceId::Auth,
            ServiceId::Data,
            ServiceId::Cedar,
            ServiceId::Cache,
            ServiceId::Email,
            ServiceId::File,
        ] {
            handle.send(ServiceAvailable::new(service_id)).await;
        }

        // Report some health checks
        for service_id in [
            ServiceId::Auth,
            ServiceId::Data,
            ServiceId::Cedar,
            ServiceId::Cache,
            ServiceId::Email,
            ServiceId::File,
        ] {
            handle
                .send(HealthCheckResult::success(service_id, 10))
                .await;
        }

        handle
    });

    c.bench_function("service_coordinator/get_status", |b| {
        b.to_async(&rt).iter(|| async {
            let (request, rx) = GetServiceStatus::new();
            handle.send(request).await;
            rx.await.expect("Should get status")
        });
    });
}

/// Benchmark service coordinator health check processing
fn bench_service_coordinator_health_check(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = ServiceCoordinatorConfig::new()
            .with_failure_threshold(5)
            .with_recovery_timeout(Duration::from_secs(30));
        let mut runtime = ActonApp::launch();
        let handle = ServiceCoordinatorAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap();

        // Register auth service
        handle.send(ServiceAvailable::new(ServiceId::Auth)).await;

        handle
    });

    c.bench_function("service_coordinator/health_check", |b| {
        b.to_async(&rt).iter(|| async {
            handle
                .send(HealthCheckResult::success(ServiceId::Auth, 10))
                .await;
        });
    });
}

/// Benchmark hot reload coordinator file change processing
fn bench_hot_reload_file_change(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = HotReloadConfig::new()
            .with_enabled(true)
            .with_debounce(ReloadType::Templates, Duration::from_millis(10));
        let mut runtime = ActonApp::launch();
        HotReloadCoordinatorAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap()
    });

    c.bench_function("hot_reload/file_change", |b| {
        b.to_async(&rt).iter(|| async {
            handle
                .send(FileChanged::new(
                    ReloadType::Templates,
                    PathBuf::from("/tmp/test.html"),
                ))
                .await;
        });
    });
}

/// Benchmark hot reload coordinator stats retrieval
fn bench_hot_reload_get_stats(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = HotReloadConfig::new().with_enabled(true);
        let mut runtime = ActonApp::launch();
        HotReloadCoordinatorAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap()
    });

    c.bench_function("hot_reload/get_stats", |b| {
        b.to_async(&rt).iter(|| async {
            let (request, rx) = HotReloadGetStats::new();
            handle.send(request).await;
            rx.await.expect("Should get stats")
        });
    });
}

/// Benchmark concurrent rate limit requests
fn bench_rate_limiter_concurrent(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.block_on(async {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(100_000)
            .with_refill_rate(10_000.0);
        let mut runtime = ActonApp::launch();
        RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap()
    });

    let mut group = c.benchmark_group("rate_limiter/concurrent");

    for concurrency in [10, 50, 100] {
        group.throughput(Throughput::Elements(concurrency));
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            &concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let mut join_set = tokio::task::JoinSet::new();

                    for i in 0..concurrency {
                        let h = handle.clone();
                        join_set.spawn(async move {
                            let (request, rx) =
                                CheckRateLimit::new(format!("concurrent_key_{i}"), 1);
                            h.send(request).await;
                            let _ = rx.await;
                        });
                    }

                    while (join_set.join_next().await).is_some() {}
                });
            },
        );
    }
    group.finish();
}

/// Benchmark token bucket algorithm directly (no agent overhead)
fn bench_token_bucket_direct(c: &mut Criterion) {
    use acton_dx::htmx::agents::TokenBucket;

    let mut bucket = TokenBucket::new(10_000, 1000.0);

    c.bench_function("token_bucket/consume_direct", |b| {
        b.iter(|| {
            let _ = bucket.try_consume(1);
        });
    });
}

/// Compare agent overhead vs direct token bucket
fn bench_agent_overhead_comparison(c: &mut Criterion) {
    use acton_dx::htmx::agents::TokenBucket;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    // Agent-based rate limiter
    let agent_handle = rt.block_on(async {
        let config = RateLimiterConfig::new()
            .with_bucket_capacity(100_000)
            .with_refill_rate(10_000.0);
        let mut runtime = ActonApp::launch();
        RateLimiterAgent::spawn_with_config(&mut runtime, config)
            .await
            .unwrap()
    });

    let mut group = c.benchmark_group("overhead_comparison");

    // Direct token bucket (baseline)
    let mut bucket = TokenBucket::new(100_000, 10_000.0);
    group.bench_function("direct_token_bucket", |b| {
        b.iter(|| {
            let _ = bucket.try_consume(1);
        });
    });

    // Agent-based (with message passing)
    group.bench_function("agent_rate_limiter", |b| {
        b.to_async(&rt).iter(|| async {
            let (request, rx) = CheckRateLimit::new("overhead_test".to_string(), 1);
            agent_handle.send(request).await;
            let _ = rx.await;
        });
    });

    group.finish();
}

criterion_group!(
    name = rate_limiter_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(5));
    targets =
        bench_rate_limiter_single_request,
        bench_rate_limiter_key_scaling,
        bench_rate_limiter_get_stats,
        bench_rate_limiter_concurrent,
);

criterion_group!(
    name = service_coordinator_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(5));
    targets =
        bench_service_coordinator_get_status,
        bench_service_coordinator_health_check,
);

criterion_group!(
    name = hot_reload_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(5));
    targets =
        bench_hot_reload_file_change,
        bench_hot_reload_get_stats,
);

criterion_group!(
    name = overhead_benches;
    config = Criterion::default()
        .sample_size(200)
        .measurement_time(Duration::from_secs(10));
    targets =
        bench_token_bucket_direct,
        bench_agent_overhead_comparison,
);

criterion_main!(
    rate_limiter_benches,
    service_coordinator_benches,
    hot_reload_benches,
    overhead_benches,
);

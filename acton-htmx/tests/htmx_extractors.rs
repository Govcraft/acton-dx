//! Integration tests for HTMX extractors
//!
//! Tests the HxRequest extractor and related HTMX header parsing.

use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use axum_htmx::{HxBoosted, HxCurrentUrl, HxHistoryRestoreRequest, HxPrompt, HxRequest, HxTarget};
use http::{header, Method, StatusCode};
use tower::ServiceExt;

/// Helper to create a test app
fn test_app() -> Router {
    Router::new()
        .route("/extract-request", get(extract_hx_request))
        .route("/extract-target", get(extract_hx_target))
        .route("/extract-boosted", get(extract_hx_boosted))
        .route("/extract-current-url", get(extract_hx_current_url))
        .route("/extract-history-restore", get(extract_hx_history_restore))
        .route("/extract-prompt", post(extract_hx_prompt))
}

// Test handlers

async fn extract_hx_request(HxRequest(is_htmx): HxRequest) -> String {
    format!("is_htmx={is_htmx}")
}

async fn extract_hx_target(HxTarget(target): HxTarget) -> String {
    format!("target={}", target.unwrap_or_else(|| "none".to_string()))
}

async fn extract_hx_boosted(HxBoosted(boosted): HxBoosted) -> String {
    format!("boosted={boosted}")
}

async fn extract_hx_current_url(HxCurrentUrl(url): HxCurrentUrl) -> String {
    format!(
        "current_url={}",
        url.map(|u| u.to_string())
            .unwrap_or_else(|| "none".to_string())
    )
}

async fn extract_hx_history_restore(
    HxHistoryRestoreRequest(is_restore): HxHistoryRestoreRequest,
) -> String {
    format!("is_restore={is_restore}")
}

async fn extract_hx_prompt(HxPrompt(prompt): HxPrompt) -> String {
    format!("prompt={}", prompt.unwrap_or_else(|| "none".to_string()))
}

// Tests

#[tokio::test]
async fn test_hx_request_with_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-request")
                .header("HX-Request", "true")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "is_htmx=true");
}

#[tokio::test]
async fn test_hx_request_without_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-request")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "is_htmx=false");
}

#[tokio::test]
async fn test_hx_target_with_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-target")
                .header("HX-Target", "#my-div")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "target=#my-div");
}

#[tokio::test]
async fn test_hx_target_without_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-target")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "target=none");
}

#[tokio::test]
async fn test_hx_boosted() {
    let app = test_app();

    // With boosted=true
    let response = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/extract-boosted")
                .header("HX-Boosted", "true")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8(body.to_vec()).unwrap(), "boosted=true");

    // Without header
    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-boosted")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8(body.to_vec()).unwrap(), "boosted=false");
}

#[tokio::test]
async fn test_hx_current_url() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-current-url")
                .header("HX-Current-URL", "https://example.com/page")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "current_url=https://example.com/page");
}

#[tokio::test]
async fn test_hx_history_restore() {
    let app = test_app();

    // With history restore
    let response = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/extract-history-restore")
                .header("HX-History-Restore-Request", "true")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8(body.to_vec()).unwrap(), "is_restore=true");

    // Without header
    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-history-restore")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8(body.to_vec()).unwrap(),
        "is_restore=false"
    );
}

#[tokio::test]
async fn test_hx_prompt() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/extract-prompt")
                .method(Method::POST)
                .header("HX-Prompt", "Enter your name")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    assert_eq!(body_str, "prompt=Enter your name");
}

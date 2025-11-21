//! Integration tests for HTMX responders
//!
//! Tests all HTMX response types and headers.

use axum::{routing::get, Router};
use axum_htmx::{
    HxLocation, HxPushUrl, HxRedirect, HxRefresh, HxReplaceUrl, HxReselect, HxReswap, HxRetarget,
    HxTrigger, HxTriggerAfterSettle, HxTriggerAfterSwap, SwapOption,
};
use http::StatusCode;
use serde_json::json;
use tower::ServiceExt;

/// Helper to create a test app
fn test_app() -> Router {
    Router::new()
        .route("/redirect", get(test_redirect))
        .route("/push-url", get(test_push_url))
        .route("/replace-url", get(test_replace_url))
        .route("/refresh", get(test_refresh))
        .route("/trigger", get(test_trigger))
        .route("/trigger-after-settle", get(test_trigger_after_settle))
        .route("/trigger-after-swap", get(test_trigger_after_swap))
        .route("/trigger-with-details", get(test_trigger_with_details))
        .route("/reswap", get(test_reswap))
        .route("/retarget", get(test_retarget))
        .route("/reselect", get(test_reselect))
        .route("/location", get(test_location))
        .route("/location-with-options", get(test_location_with_options))
}

// Test handlers

async fn test_redirect() -> HxRedirect {
    HxRedirect::to("/new-page")
}

async fn test_push_url() -> (HxPushUrl, &'static str) {
    (HxPushUrl::from("/new-url"), "content")
}

async fn test_replace_url() -> (HxReplaceUrl, &'static str) {
    (HxReplaceUrl::from("/replaced-url"), "content")
}

async fn test_refresh() -> HxRefresh {
    HxRefresh
}

async fn test_trigger() -> (HxTrigger, &'static str) {
    (HxTrigger::normal(["myEvent"]), "content")
}

async fn test_trigger_after_settle() -> (HxTriggerAfterSettle, &'static str) {
    (HxTriggerAfterSettle::normal(["settleEvent"]), "content")
}

async fn test_trigger_after_swap() -> (HxTriggerAfterSwap, &'static str) {
    (HxTriggerAfterSwap::normal(["swapEvent"]), "content")
}

async fn test_trigger_with_details() -> (HxTrigger, &'static str) {
    let details = json!({"message": "Hello from server"});
    (
        HxTrigger::new([("detailedEvent".to_string(), details)]),
        "content",
    )
}

async fn test_reswap() -> (HxReswap, &'static str) {
    (HxReswap::from(SwapOption::OuterHtml), "content")
}

async fn test_retarget() -> (HxRetarget, &'static str) {
    (HxRetarget::from("#new-target"), "content")
}

async fn test_reselect() -> (HxReselect, &'static str) {
    (HxReselect::from("#selected-content"), "content")
}

async fn test_location() -> (HxLocation, &'static str) {
    (HxLocation::from("/location"), "content")
}

async fn test_location_with_options() -> (HxLocation, &'static str) {
    use axum_htmx::LocationOptions;

    let options = LocationOptions {
        source: None,
        event: None,
        target: None,
        swap: None,
        values: Some(json!({"message": "context"})),
        headers: None,
    };

    (
        HxLocation::from_uri_with_options("/location", options),
        "content",
    )
}

// Tests

#[tokio::test]
async fn test_hx_redirect_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/redirect")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers().get("HX-Redirect").unwrap(), "/new-page");
}

#[tokio::test]
async fn test_hx_push_url_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/push-url")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("HX-Push-Url").unwrap(), "/new-url");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8(body.to_vec()).unwrap(), "content");
}

#[tokio::test]
async fn test_hx_replace_url_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/replace-url")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("HX-Replace-Url").unwrap(),
        "/replaced-url"
    );
}

#[tokio::test]
async fn test_hx_refresh_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/refresh")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("HX-Refresh").unwrap(), "true");
}

#[tokio::test]
async fn test_hx_trigger_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/trigger")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("HX-Trigger"));

    let trigger = response.headers().get("HX-Trigger").unwrap();
    let trigger_str = trigger.to_str().unwrap();

    // Should contain myEvent (exact format may vary)
    assert!(trigger_str.contains("myEvent"));
}

#[tokio::test]
async fn test_hx_trigger_after_settle_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/trigger-after-settle")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("HX-Trigger-After-Settle"));

    let trigger = response.headers().get("HX-Trigger-After-Settle").unwrap();
    assert!(trigger.to_str().unwrap().contains("settleEvent"));
}

#[tokio::test]
async fn test_hx_trigger_after_swap_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/trigger-after-swap")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("HX-Trigger-After-Swap"));

    let trigger = response.headers().get("HX-Trigger-After-Swap").unwrap();
    assert!(trigger.to_str().unwrap().contains("swapEvent"));
}

#[tokio::test]
async fn test_hx_trigger_with_details_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/trigger-with-details")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("HX-Trigger"));

    let trigger = response.headers().get("HX-Trigger").unwrap();
    let trigger_str = trigger.to_str().unwrap();

    // Should contain event name and details in JSON format
    assert!(trigger_str.contains("detailedEvent"));
    assert!(trigger_str.contains("message"));
}

#[tokio::test]
async fn test_hx_reswap_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/reswap")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("HX-Reswap").unwrap(), "outerHTML");
}

#[tokio::test]
async fn test_hx_retarget_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/retarget")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("HX-Retarget").unwrap(),
        "#new-target"
    );
}

#[tokio::test]
async fn test_hx_reselect_header() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/reselect")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("HX-Reselect").unwrap(),
        "#selected-content"
    );
}

#[tokio::test]
async fn test_hx_location_header() {
    let app = test_app();

    let response = app
        .clone()
        .oneshot(
            http::Request::builder()
                .uri("/location")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("HX-Location"));

    let location = response.headers().get("HX-Location").unwrap();
    assert!(location.to_str().unwrap().contains("/location"));
}

#[tokio::test]
async fn test_hx_location_with_context() {
    let app = test_app();

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/location-with-options")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("HX-Location"));

    let location = response.headers().get("HX-Location").unwrap();
    let location_str = location.to_str().unwrap();

    // Should contain path and context
    assert!(location_str.contains("/location"));
    assert!(location_str.contains("message"));
}

#[tokio::test]
async fn test_multiple_response_headers() {
    // Test combining multiple HTMX headers

    async fn multi_header_handler() -> (HxTrigger, HxPushUrl, &'static str) {
        (
            HxTrigger::normal(["event1", "event2"]),
            HxPushUrl::from("/new-path"),
            "Multi-header content",
        )
    }

    let app = Router::new().route("/multi", get(multi_header_handler));

    let response = app
        .oneshot(
            http::Request::builder()
                .uri("/multi")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("HX-Trigger"));
    assert!(response.headers().contains_key("HX-Push-Url"));

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8(body.to_vec()).unwrap(),
        "Multi-header content"
    );
}

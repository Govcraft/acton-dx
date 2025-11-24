//! Shared HTTP client for OAuth2 providers
//!
//! This module provides a unified async HTTP client implementation that all OAuth2
//! providers use for token exchange requests. By centralizing this logic, we ensure
//! consistent behavior, simplify testing, and reduce code duplication.

/// Async HTTP client for OAuth2 requests
///
/// This function is used by all OAuth2 providers to perform HTTP requests during
/// the token exchange flow. It uses `reqwest` with disabled redirects (as required
/// by the OAuth2 spec) and properly converts between `oauth2::HttpRequest` and
/// `oauth2::HttpResponse` types.
///
/// # Errors
///
/// Returns `reqwest::Error` if:
/// - The HTTP client cannot be built
/// - The HTTP request fails to send
/// - The response body cannot be read
///
/// # Panics
///
/// Should never panic in practice. The response builder is constructed with valid
/// components (status code, headers, body) and will only panic if these are invalid,
/// which cannot happen given our usage.
///
/// # Implementation Notes
///
/// - Redirects are disabled (`Policy::none`) per OAuth2 specification
/// - All request headers are preserved and forwarded
/// - Response bodies are fully buffered before returning
pub async fn async_http_client(
    request: oauth2::HttpRequest,
) -> Result<oauth2::HttpResponse, reqwest::Error> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let method = request.method().clone();
    let url = request.uri().to_string();
    let headers = request.headers().clone();
    let body = request.into_body();

    let mut request_builder = client.request(method, &url).body(body);

    for (name, value) in &headers {
        request_builder = request_builder.header(name.as_str(), value.as_bytes());
    }

    let response = request_builder.send().await?;

    let status_code = response.status();
    let headers = response.headers().to_owned();
    let body = response.bytes().await?.to_vec();

    let mut builder = http::Response::builder().status(status_code);
    for (name, value) in &headers {
        builder = builder.header(name, value);
    }

    // This should never fail as we're building with valid components
    Ok(builder
        .body(body)
        .expect("Failed to build HTTP response"))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_async_http_client_compiles() {
        // This test primarily ensures the function compiles and has correct types
        // Actual HTTP testing would require mocking or integration tests

        // Test passes by virtue of compilation
    }
}

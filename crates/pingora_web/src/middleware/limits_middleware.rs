use async_trait::async_trait;
use http::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use super::Middleware;
use crate::core::{Request, Response, router::Handler};

/// Configuration for timeout and size limits
#[derive(Clone)]
pub struct LimitsConfig {
    /// Maximum request timeout (default: 30 seconds)
    pub request_timeout: Duration,
    /// Maximum request body size in bytes (default: 1MB)
    pub max_body_size: usize,
    /// Maximum URL path length (default: 2048 characters)
    pub max_path_length: usize,
    /// Maximum number of headers (default: 100)
    pub max_headers: usize,
    /// Maximum single header value size (default: 8KB)
    pub max_header_size: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            max_body_size: 1024 * 1024, // 1MB
            max_path_length: 2048,
            max_headers: 100,
            max_header_size: 8 * 1024, // 8KB
        }
    }
}

impl LimitsConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set request timeout
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Set maximum body size
    pub fn max_body_size(mut self, size: usize) -> Self {
        self.max_body_size = size;
        self
    }

    /// Set maximum path length
    pub fn max_path_length(mut self, length: usize) -> Self {
        self.max_path_length = length;
        self
    }

    /// Set maximum number of headers
    pub fn max_headers(mut self, count: usize) -> Self {
        self.max_headers = count;
        self
    }

    /// Set maximum header value size
    pub fn max_header_size(mut self, size: usize) -> Self {
        self.max_header_size = size;
        self
    }
}

/// Middleware for enforcing global timeout and size limits
pub struct LimitsMiddleware {
    config: LimitsConfig,
}

impl LimitsMiddleware {
    /// Create new limits middleware with default configuration
    pub fn new() -> Self {
        Self {
            config: LimitsConfig::default(),
        }
    }

    /// Create new limits middleware with custom configuration
    pub fn with_config(config: LimitsConfig) -> Self {
        Self { config }
    }

    /// Validate request limits before processing
    fn validate_request(&self, req: &Request) -> Option<Response> {
        // Check path length
        if req.path().len() > self.config.max_path_length {
            tracing::warn!(
                "Request path too long: {} > {}",
                req.path().len(),
                self.config.max_path_length
            );
            return Some(Response::text(StatusCode::URI_TOO_LONG, "URI Too Long"));
        }

        // Check number of headers
        if req.headers().len() > self.config.max_headers {
            tracing::warn!(
                "Too many headers: {} > {}",
                req.headers().len(),
                self.config.max_headers
            );
            return Some(Response::text(
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                "Request Header Fields Too Large",
            ));
        }

        // Check individual header sizes
        for (name, value) in req.headers() {
            let name_len = name.as_str().len();
            let value_len = value.len();
            if name_len + value_len > self.config.max_header_size {
                tracing::warn!(
                    "Header too large: {} + {} > {}",
                    name_len,
                    value_len,
                    self.config.max_header_size
                );
                return Some(Response::text(
                    StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                    "Request Header Fields Too Large",
                ));
            }
        }

        // Check body size
        if req.body().len() > self.config.max_body_size {
            tracing::warn!(
                "Request body too large: {} > {}",
                req.body().len(),
                self.config.max_body_size
            );
            return Some(Response::text(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Payload Too Large",
            ));
        }

        None
    }
}

impl Default for LimitsMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for LimitsMiddleware {
    async fn handle(&self, req: Request, next: Arc<dyn Handler>) -> Response {
        // First validate request limits
        if let Some(error_response) = self.validate_request(&req) {
            return error_response;
        }

        // Apply timeout to the entire request processing
        match timeout(self.config.request_timeout, next.handle(req)).await {
            Ok(response) => response,
            Err(_) => {
                tracing::warn!(
                    "Request timeout after {}ms",
                    self.config.request_timeout.as_millis()
                );
                Response::text(StatusCode::REQUEST_TIMEOUT, "Request Timeout")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Method, Request};

    struct MockHandler {
        delay: Option<Duration>,
    }

    impl MockHandler {
        fn new() -> Arc<Self> {
            Arc::new(Self { delay: None })
        }

        fn with_delay(delay: Duration) -> Arc<Self> {
            Arc::new(Self { delay: Some(delay) })
        }
    }

    #[async_trait]
    impl Handler for MockHandler {
        async fn handle(&self, _req: Request) -> Response {
            if let Some(delay) = self.delay {
                tokio::time::sleep(delay).await;
            }
            Response::text(StatusCode::OK, "ok")
        }
    }

    #[tokio::test]
    async fn test_request_timeout() {
        let config = LimitsConfig::new().request_timeout(Duration::from_millis(100));
        let middleware = LimitsMiddleware::with_config(config);

        let handler = MockHandler::with_delay(Duration::from_millis(200));
        let req = Request::new(Method::GET, "/test");

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 408);
    }

    #[tokio::test]
    async fn test_path_length_limit() {
        let config = LimitsConfig::new().max_path_length(10);
        let middleware = LimitsMiddleware::with_config(config);

        let handler = MockHandler::new();
        let req = Request::new(Method::GET, "/very-long-path-that-exceeds-limit");

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 414);
    }

    #[tokio::test]
    async fn test_body_size_limit() {
        let config = LimitsConfig::new().max_body_size(5);
        let middleware = LimitsMiddleware::with_config(config);

        let handler = MockHandler::new();
        let req = Request::new(Method::POST, "/test").with_body(b"too long body".to_vec());

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 413);
    }

    #[tokio::test]
    async fn test_header_count_limit() {
        let config = LimitsConfig::new().max_headers(2);
        let middleware = LimitsMiddleware::with_config(config);

        let handler = MockHandler::new();
        let mut req = Request::new(Method::GET, "/test");
        req.headers_mut()
            .insert("header1", "value1".try_into().unwrap());
        req.headers_mut()
            .insert("header2", "value2".try_into().unwrap());
        req.headers_mut()
            .insert("header3", "value3".try_into().unwrap());

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 431);
    }

    #[tokio::test]
    async fn test_header_size_limit() {
        let config = LimitsConfig::new().max_header_size(10);
        let middleware = LimitsMiddleware::with_config(config);

        let handler = MockHandler::new();
        let mut req = Request::new(Method::GET, "/test");
        req.headers_mut()
            .insert("x-long", "very-long-value".try_into().unwrap());

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 431);
    }

    #[tokio::test]
    async fn test_valid_request_passes() {
        let config = LimitsConfig::new();
        let middleware = LimitsMiddleware::with_config(config);

        let handler = MockHandler::new();
        let req = Request::new(Method::GET, "/test").with_body(b"small".to_vec());

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 200);
    }
}

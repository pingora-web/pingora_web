use async_trait::async_trait;
use futures::FutureExt;
use http::StatusCode;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use super::Middleware;
use crate::core::{Handler, PingoraHttpRequest, PingoraWebHttpResponse};
use crate::error::{ResponseError, WebError};

/// Simple panic recovery middleware that catches panics and returns 500 errors
pub struct PanicRecoveryMiddleware;

impl PanicRecoveryMiddleware {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PanicRecoveryMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for PanicRecoveryMiddleware {
    async fn handle(
        &self,
        req: PingoraHttpRequest,
        next: Arc<dyn Handler>,
    ) -> Result<PingoraWebHttpResponse, WebError> {
        // Wrap the next handler call in a catch_unwind
        let result = AssertUnwindSafe(next.handle(req)).catch_unwind().await;

        match result {
            Ok(handler_result) => handler_result,
            Err(panic_info) => {
                // Extract panic message
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic occurred".to_string()
                };

                // Create panic error
                let panic_error = PanicError::new(panic_msg);
                Err(WebError::new(panic_error))
            }
        }
    }
}

/// Error type for panics
#[derive(Debug)]
struct PanicError {
    message: String,
}

impl PanicError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for PanicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Panic: {}", self.message)
    }
}

impl std::error::Error for PanicError {}

impl ResponseError for PanicError {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Method, PingoraHttpRequest};

    struct PanicHandler;

    #[async_trait]
    impl Handler for PanicHandler {
        async fn handle(
            &self,
            _req: PingoraHttpRequest,
        ) -> Result<PingoraWebHttpResponse, WebError> {
            panic!("Test panic message");
        }
    }

    struct NormalHandler;

    #[async_trait]
    impl Handler for NormalHandler {
        async fn handle(
            &self,
            _req: PingoraHttpRequest,
        ) -> Result<PingoraWebHttpResponse, WebError> {
            Ok(PingoraWebHttpResponse::text(StatusCode::OK, "ok"))
        }
    }

    #[tokio::test]
    async fn test_panic_recovery() {
        let middleware = PanicRecoveryMiddleware::new();
        let handler = Arc::new(PanicHandler);
        let req = PingoraHttpRequest::new(Method::GET, "/test");

        let result = middleware.handle(req, handler).await;
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(
                error.as_response_error().status_code(),
                StatusCode::INTERNAL_SERVER_ERROR
            );
        }
    }

    #[tokio::test]
    async fn test_normal_request_passes_through() {
        let middleware = PanicRecoveryMiddleware::new();
        let handler = Arc::new(NormalHandler);
        let req = PingoraHttpRequest::new(Method::GET, "/test");

        let result = middleware.handle(req, handler).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status.as_u16(), 200);
    }
}

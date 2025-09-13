use async_trait::async_trait;
use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use super::Middleware;
use crate::core::{Request, Response, router::Handler};

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
    async fn handle(&self, req: Request, next: Arc<dyn Handler>) -> Response {
        // Wrap the next handler call in a catch_unwind
        let result = AssertUnwindSafe(next.handle(req)).catch_unwind().await;

        result.unwrap_or_else(|panic_info| {
            // Extract panic message if possible
            let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            // Log the panic
            tracing::error!("Panic caught in request handler: {}", panic_msg);

            // Return a 500 Internal Server Error
            Response::text(500, "Internal Server Error")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Method, Request};

    struct PanicHandler;

    #[async_trait]
    impl Handler for PanicHandler {
        async fn handle(&self, _req: Request) -> Response {
            panic!("Test panic message");
        }
    }

    struct NormalHandler;

    #[async_trait]
    impl Handler for NormalHandler {
        async fn handle(&self, _req: Request) -> Response {
            Response::text(200, "ok")
        }
    }

    #[tokio::test]
    async fn test_panic_recovery() {
        let middleware = PanicRecoveryMiddleware::new();
        let handler = Arc::new(PanicHandler);
        let req = Request::new(Method::GET, "/test");

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 500);
    }

    #[tokio::test]
    async fn test_normal_request_passes_through() {
        let middleware = PanicRecoveryMiddleware::new();
        let handler = Arc::new(NormalHandler);
        let req = Request::new(Method::GET, "/test");

        let response = middleware.handle(req, handler).await;
        assert_eq!(response.status.as_u16(), 200);
    }
}

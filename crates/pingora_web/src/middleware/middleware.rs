use async_trait::async_trait;
use std::sync::Arc;

use crate::core::{Handler, PingoraHttpRequest, PingoraWebHttpResponse};
use crate::error::WebError;

/// Middleware trait for processing requests
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    /// Process the request, optionally calling the next handler
    async fn handle(
        &self,
        req: PingoraHttpRequest,
        next: Arc<dyn Handler>,
    ) -> Result<PingoraWebHttpResponse, WebError>;
}

/// Wrapper that implements Handler for middleware composition
struct MiddlewareHandler {
    middleware: Arc<dyn Middleware>,
    next: Arc<dyn Handler>,
}

#[async_trait]
impl Handler for MiddlewareHandler {
    async fn handle(&self, req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        self.middleware.handle(req, Arc::clone(&self.next)).await
    }
}

/// Compose multiple middlewares around a final handler
/// Creates an onion model where the last middleware wraps all previous ones
pub fn compose(
    middlewares: &[Arc<dyn Middleware>],
    final_handler: Arc<dyn Handler>,
) -> Arc<dyn Handler> {
    let mut current_handler = final_handler;

    // 从后往前遍历中间件，让后注册的中间件在外层
    for i in (0..middlewares.len()).rev() {
        let middleware = Arc::clone(&middlewares[i]);
        let next_handler = Arc::clone(&current_handler);

        // 创建一个新的处理器，将当前中间件包装在外层
        current_handler = Arc::new(MiddlewareHandler {
            middleware,
            next: next_handler,
        });
    }

    current_handler
}

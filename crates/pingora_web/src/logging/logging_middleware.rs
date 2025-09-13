use crate::core::router::Handler;
use crate::{
    core::{Request, Response},
    logging::{Level, Logger},
    middleware::Middleware,
};
use async_trait::async_trait;
use std::sync::Arc;

/// Logging middleware that measures request latency and logs completion
pub struct LoggingMiddleware {
    logger: Arc<dyn Logger>,
}

impl LoggingMiddleware {
    pub fn new<L: Logger + 'static>(logger: L) -> Self {
        Self {
            logger: Arc::new(logger),
        }
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn handle(&self, req: Request, next: Arc<dyn Handler>) -> Response {
        let start_time = std::time::Instant::now();
        let method = req.method().clone();
        let path = req.path().to_string();

        let res = next.handle(req).await;

        let elapsed = start_time.elapsed().as_millis();
        let request_id = res
            .headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let msg = format!(
            "{} {} -> {} in {}ms",
            method.as_str(),
            path,
            res.status.as_u16(),
            elapsed
        );
        self.logger.log(Level::Info, &msg, request_id);

        res
    }
}

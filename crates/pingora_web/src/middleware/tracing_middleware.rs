use crate::core::Handler;
use crate::{
    core::{PingoraHttpRequest, PingoraWebHttpResponse},
    error::WebError,
    middleware::Middleware,
};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{Instrument, info};

/// Tracing middleware that creates a span for each request with request_id context
/// This ensures all tracing calls within the request have the request_id automatically included
#[derive(Clone)]
pub struct TracingMiddleware;

impl TracingMiddleware {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TracingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for TracingMiddleware {
    async fn handle(
        &self,
        req: PingoraHttpRequest,
        next: Arc<dyn Handler>,
    ) -> Result<PingoraWebHttpResponse, WebError> {
        let request_id = req
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let method = req.method().as_str().to_string();
        let path = req.path().to_string();

        // Create a span for this request with structured fields
        let span = tracing::info_span!(
            "request",
            request_id = request_id.as_str(),
            method = method.as_str(),
            path = path,
            status = tracing::field::Empty,
            latency_ms = tracing::field::Empty,
        );

        // Clone span for use in both the closure and instrument
        let span_for_record = span.clone();

        // Execute the rest of the middleware chain within this span
        async move {
            // Log request start
            info!("Request started");

            let start_time = std::time::Instant::now();

            let res = next.handle(req).await?;

            let elapsed_ms = start_time.elapsed().as_millis();

            // Record the response status and latency in the span
            span_for_record.record("status", res.status.as_u16());
            span_for_record.record("latency_ms", elapsed_ms);

            // Log the request completion
            info!("Request completed");

            Ok(res)
        }
        .instrument(span)
        .await
    }
}

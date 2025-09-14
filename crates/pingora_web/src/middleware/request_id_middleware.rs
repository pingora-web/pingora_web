use crate::{
    core::Handler,
    core::{PingoraHttpRequest, PingoraWebHttpResponse},
    error::WebError,
    middleware::Middleware,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct RequestId {
    header: &'static str,
}

impl RequestId {
    pub fn new() -> Self {
        Self {
            header: "x-request-id",
        }
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Middleware for RequestId {
    async fn handle(
        &self,
        mut req: PingoraHttpRequest,
        next: Arc<dyn Handler>,
    ) -> Result<PingoraWebHttpResponse, WebError> {
        // Generate or use existing request ID
        let request_id = req
            .headers()
            .get(self.header)
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(crate::utils::request_id::generate);

        // Store request ID in request headers for later access
        let _ = req.headers_mut().insert(
            self.header,
            http::HeaderValue::from_str(&request_id).unwrap(),
        );

        let mut res = next.handle(req).await?;

        // Ensure response has the request ID header
        if !res.headers.contains_key(self.header) {
            let _ = res.headers.insert(
                self.header,
                http::HeaderValue::from_str(&request_id).unwrap(),
            );
        }
        Ok(res)
    }
}

use super::*;
use crate::core::{Method, Request, Response, response::Body, router::Handler};
use async_trait::async_trait;
use flate2::read::GzDecoder;
use std::io::Read;

struct MockHandler {
    body: bytes::Bytes,
    status: u16,
    headers: http::HeaderMap,
}

impl MockHandler {
    fn new(response: Response) -> Arc<Self> {
        let body = match response.body {
            Body::Bytes(bytes) => bytes,
            Body::Stream(_) => bytes::Bytes::from(b"stream content".to_vec()),
        };
        Arc::new(Self {
            body,
            status: response.status.as_u16(),
            headers: response.headers,
        })
    }
}

#[async_trait]
impl Handler for MockHandler {
    async fn handle(&self, _req: Request) -> Response {
        let mut response = Response::bytes(self.status, self.body.clone());
        response.headers = self.headers.clone();
        response
    }
}

#[tokio::test]
async fn test_compresses_text_response() {
    let middleware = CompressionMiddleware::new();
    let large_text = "x".repeat(2000); // Larger than min_size
    let response = Response::text(200, &large_text);
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/test");
    req.headers_mut()
        .insert("accept-encoding", "gzip".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should be compressed
    assert_eq!(
        result
            .headers
            .get(http::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
    assert_eq!(
        result
            .headers
            .get(http::header::VARY)
            .and_then(|v| v.to_str().ok()),
        Some("Accept-Encoding")
    );
    assert!(!result.headers.contains_key(http::header::CONTENT_LENGTH));

    // Verify compressed content can be decompressed
    if let Body::Bytes(compressed) = result.body {
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert_eq!(decompressed, large_text);
    } else {
        panic!("Expected bytes body");
    }
}

#[tokio::test]
async fn test_skips_small_response() {
    let config = CompressionConfig::new().min_size(1000);
    let middleware = CompressionMiddleware::with_config(config);
    let small_text = "small";
    let response = Response::text(200, small_text);
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/test");
    req.headers_mut()
        .insert("accept-encoding", "gzip".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should not be compressed
    assert!(!result.headers.contains_key(http::header::CONTENT_ENCODING));
    if let Body::Bytes(body) = result.body {
        assert_eq!(std::str::from_utf8(&body).unwrap(), small_text);
    }
}

#[tokio::test]
async fn test_skips_unsupported_content_type() {
    let middleware = CompressionMiddleware::new();
    let response = Response::bytes(200, b"binary data".repeat(200));
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/test");
    req.headers_mut()
        .insert("accept-encoding", "gzip".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should not be compressed (no content-type or unsupported type)
    assert!(!result.headers.contains_key("content-encoding"));
}

#[tokio::test]
async fn test_skips_when_client_doesnt_support_gzip() {
    let middleware = CompressionMiddleware::new();
    let large_text = "x".repeat(2000);
    let response = Response::text(200, &large_text);
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/test");
    req.headers_mut()
        .insert("accept-encoding", "deflate".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should not be compressed (client doesn't accept gzip)
    assert!(!result.headers.contains_key(http::header::CONTENT_ENCODING));
    if let Body::Bytes(body) = result.body {
        assert_eq!(std::str::from_utf8(&body).unwrap(), large_text);
    }
}

#[tokio::test]
async fn test_compresses_json_response() {
    let middleware = CompressionMiddleware::new();
    let json_data = serde_json::json!({
        "data": "x".repeat(2000),
        "status": "ok"
    });
    let response = Response::json(200, &json_data);
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/api/data");
    req.headers_mut()
        .insert("accept-encoding", "gzip".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should be compressed
    assert_eq!(
        result
            .headers
            .get(http::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
    assert_eq!(
        result
            .headers
            .get(http::header::VARY)
            .and_then(|v| v.to_str().ok()),
        Some("Accept-Encoding")
    );
}

#[tokio::test]
async fn test_respects_existing_content_encoding() {
    let middleware = CompressionMiddleware::new();
    let large_text = "x".repeat(2000);
    let mut response = Response::text(200, &large_text);
    response.headers.insert(
        http::header::CONTENT_ENCODING,
        http::HeaderValue::from_static("br"),
    );
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/test");
    req.headers_mut()
        .insert("accept-encoding", "gzip".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should not be compressed (already has content-encoding)
    assert_eq!(
        result
            .headers
            .get(http::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok()),
        Some("br")
    );
}

#[tokio::test]
async fn test_compress_all_types_when_filtering_disabled() {
    let config = CompressionConfig::new().compress_all_types(); // Disable content type filtering
    let middleware = CompressionMiddleware::with_config(config);

    // Binary data that normally wouldn't be compressed
    let binary_data = b"binary data".repeat(200);
    let response = Response::bytes(200, binary_data.clone());
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/binary");
    req.headers_mut()
        .insert("accept-encoding", "gzip".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should be compressed because content type filtering is disabled
    assert_eq!(
        result
            .headers
            .get(http::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
    assert_eq!(
        result
            .headers
            .get(http::header::VARY)
            .and_then(|v| v.to_str().ok()),
        Some("Accept-Encoding")
    );
}

#[tokio::test]
async fn test_content_type_filtering_enabled() {
    let config = CompressionConfig::new().filter_content_types(true);
    let middleware = CompressionMiddleware::with_config(config);

    // Binary data that shouldn't be compressed with filtering enabled
    let binary_data = b"binary data".repeat(200);
    let response = Response::bytes(200, binary_data.clone());
    let handler = MockHandler::new(response);

    let mut req = Request::new(Method::GET, "/binary");
    req.headers_mut()
        .insert("accept-encoding", "gzip".try_into().unwrap());

    let result = middleware.handle(req, handler).await;

    // Should not be compressed (no content-type or unsupported type)
    assert!(!result.headers.contains_key(http::header::CONTENT_ENCODING));
    if let Body::Bytes(body) = result.body {
        assert_eq!(body.as_ref(), binary_data.as_slice());
    }
}

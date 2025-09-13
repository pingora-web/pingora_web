use async_trait::async_trait;
use std::sync::Arc;
use std::io::Write;
use flate2::{write::GzEncoder, Compression};
use futures::{stream::BoxStream, StreamExt};

use crate::core::{Request, Response, response::Body, router::Handler};
use super::Middleware;

/// Compression algorithms supported by the middleware
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// Gzip compression (widely supported, good compression)
    Gzip,
}

impl CompressionAlgorithm {
    fn encoding_name(&self) -> &'static str {
        match self {
            CompressionAlgorithm::Gzip => "gzip",
        }
    }
}

/// Configuration for compression middleware
#[derive(Clone)]
pub struct CompressionConfig {
    /// Compression level (0-9 for gzip/deflate, 0-11 for brotli)
    pub level: u32,
    /// Minimum response size to compress (bytes)
    pub min_size: usize,
    /// Supported algorithms in order of preference
    pub algorithms: Vec<CompressionAlgorithm>,
    /// Content types that should be compressed (empty means compress all types)
    pub compress_types: Vec<String>,
    /// Whether to enable content type filtering (if false, compress all content types)
    pub filter_content_types: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: 6, // Default compression level
            min_size: 1024, // Only compress responses >= 1KB
            algorithms: vec![CompressionAlgorithm::Gzip],
            compress_types: vec![
                "text/".to_string(),
                "application/json".to_string(),
                "application/javascript".to_string(),
                "application/xml".to_string(),
                "application/rss+xml".to_string(),
                "application/atom+xml".to_string(),
                "image/svg+xml".to_string(),
            ],
            filter_content_types: true, // Enable content type filtering by default
        }
    }
}

impl CompressionConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set compression level (0-9)
    pub fn level(mut self, level: u32) -> Self {
        self.level = level.min(9);
        self
    }

    /// Set minimum size threshold for compression
    pub fn min_size(mut self, size: usize) -> Self {
        self.min_size = size;
        self
    }

    /// Add a content type pattern that should be compressed
    pub fn compress_type<S: Into<String>>(mut self, content_type: S) -> Self {
        self.compress_types.push(content_type.into());
        self
    }

    /// Set the list of compression algorithms in order of preference
    pub fn algorithms(mut self, algorithms: Vec<CompressionAlgorithm>) -> Self {
        self.algorithms = algorithms;
        self
    }

    /// Enable or disable content type filtering
    pub fn filter_content_types(mut self, filter: bool) -> Self {
        self.filter_content_types = filter;
        self
    }

    /// Disable content type filtering (compress all types)
    pub fn compress_all_types(mut self) -> Self {
        self.filter_content_types = false;
        self
    }
}

/// Middleware for HTTP response compression
pub struct CompressionMiddleware {
    config: CompressionConfig,
}

impl CompressionMiddleware {
    /// Create new compression middleware with default configuration
    pub fn new() -> Self {
        Self {
            config: CompressionConfig::default(),
        }
    }

    /// Create new compression middleware with custom configuration
    pub fn with_config(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Check if the client accepts the given encoding
    fn accepts_encoding(&self, req: &Request, encoding: &str) -> bool {
        req.headers()
            .get("accept-encoding")
            .and_then(|v| v.to_str().ok())
            .map(|accept_encoding| {
                accept_encoding
                    .split(',')
                    .map(|s| s.trim())
                    .any(|enc| enc.eq_ignore_ascii_case(encoding) || enc.eq_ignore_ascii_case("*"))
            })
            .unwrap_or(false)
    }

    /// Choose the best compression algorithm based on client support
    fn choose_algorithm(&self, req: &Request) -> Option<CompressionAlgorithm> {
        self
            .config
            .algorithms
            .iter()
            .find(|&&algorithm| self.accepts_encoding(req, algorithm.encoding_name()))
            .copied()
    }

    /// Check if the content type should be compressed
    fn should_compress_content_type(&self, content_type: &str) -> bool {
        // If content type filtering is disabled, compress all types
        if !self.config.filter_content_types {
            return true;
        }

        // If no content types are specified, compress all types
        if self.config.compress_types.is_empty() {
            return true;
        }

        // Check if content type matches any pattern
        for pattern in &self.config.compress_types {
            if content_type.starts_with(pattern) {
                return true;
            }
        }
        false
    }

    /// Check if the response (alone) allows compression according to config
    fn response_allows_compress(&self, res: &Response) -> bool {
        // Don't compress if content-encoding is already set
        if res.headers.contains_key(http::header::CONTENT_ENCODING) {
            return false;
        }

        // Check content type
        if let Some(content_type) = res.headers.get(http::header::CONTENT_TYPE) {
            if let Ok(ct) = content_type.to_str() {
                if !self.should_compress_content_type(ct) {
                    return false;
                }
            } else {
                return false;
            }
        } else if self.config.filter_content_types {
            // No content-type header, only compress if content type filtering is disabled
            return false;
        }

        // For byte bodies, check size threshold
        if let Body::Bytes(bytes) = &res.body
            && bytes.len() < self.config.min_size
        {
            return false;
        }

        true
    }

    /// Compress byte data using the specified algorithm
    fn compress_bytes(&self, data: &[u8], algorithm: CompressionAlgorithm) -> Result<Vec<u8>, std::io::Error> {
        match algorithm {
            CompressionAlgorithm::Gzip => {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.config.level));
                encoder.write_all(data)?;
                encoder.finish()
            }
        }
    }

    /// Create a compressed stream from an uncompressed stream
    fn compress_stream(
        &self,
        stream: BoxStream<'static, bytes::Bytes>,
        algorithm: CompressionAlgorithm,
    ) -> BoxStream<'static, bytes::Bytes> {
        match algorithm {
            CompressionAlgorithm::Gzip => {
                Box::pin(futures::stream::unfold(
                    (stream, None, false),
                    |(mut stream, mut encoder_opt, finished)| async move {
                        if finished {
                            return None;
                        }

                        // Initialize encoder on first chunk
                        if encoder_opt.is_none() {
                            encoder_opt = Some(GzEncoder::new(Vec::new(), Compression::new(6)));
                        }

                        let mut encoder = encoder_opt.take().unwrap();

                        match stream.next().await {
                            Some(chunk) => {
                                // Write chunk to encoder
                                if encoder.write_all(&chunk).is_err() {
                                    return None;
                                }

                                // Take compressed data so far without allocating
                                let compressed = std::mem::take(encoder.get_mut());

                                Some((bytes::Bytes::from(compressed), (stream, Some(encoder), false)))
                            }
                            None => {
                                // Finish compression
                                match encoder.finish() {
                                    Ok(final_data) => {
                                        Some((bytes::Bytes::from(final_data), (stream, None, true)))
                                    }
                                    Err(_) => None,
                                }
                            }
                        }
                    }
                ))
            }
        }
    }
}

impl Default for CompressionMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for CompressionMiddleware {
    async fn handle(&self, req: Request, next: Arc<dyn Handler>) -> Response {
        // Pre-compute the best algorithm from the request, then move request downstream
        let algo = self.choose_algorithm(&req);
        let mut response = next.handle(req).await;

        // Check if we should compress this response
        if let Some(algorithm) = algo.filter(|_| self.response_allows_compress(&response)) {
            match response.body {
                Body::Bytes(ref bytes) => {
                    // Compress byte body
                    if let Ok(compressed) = self.compress_bytes(bytes, algorithm) {
                        response.body = Body::Bytes(bytes::Bytes::from(compressed));
                        let _ = response.headers.insert(
                            http::header::CONTENT_ENCODING,
                            http::HeaderValue::from_str(algorithm.encoding_name()).unwrap(),
                        );
                        let _ = response.headers.insert(
                            http::header::VARY,
                            http::HeaderValue::from_static("Accept-Encoding"),
                        );

                        // Remove content-length since compression changes the size
                        response.headers.remove(http::header::CONTENT_LENGTH);
                    }
                }
                Body::Stream(stream) => {
                    // Compress streaming body
                    let compressed_stream = self.compress_stream(stream, algorithm);
                    response.body = Body::Stream(compressed_stream);
                    let _ = response.headers.insert(
                        http::header::CONTENT_ENCODING,
                        http::HeaderValue::from_str(algorithm.encoding_name()).unwrap(),
                    );
                    let _ = response.headers.insert(
                        http::header::VARY,
                        http::HeaderValue::from_static("Accept-Encoding"),
                    );

                    // Remove content-length for streaming responses with compression
                    response.headers.remove(http::header::CONTENT_LENGTH);
                }
            }
        }

        response
    }
}

#[cfg(test)]
mod tests;

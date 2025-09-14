use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use http::StatusCode;

use crate::core::Handler;
use crate::core::{PingoraHttpRequest, PingoraWebHttpResponse};
use crate::error::WebError;

/// Serve static files from a directory, similar to axum's ServeDir.
///
/// Usage:
///   router.get("/assets/*path", Arc::new(ServeDir::new("assets")));
///
/// Security: performs simple path normalization to prevent path traversal.
pub struct ServeDir {
    root: PathBuf,
    // Optional route param name to read relative path from (e.g. "path", "file").
    // If None, will try common defaults then fall back to the only param (if exactly one).
    param: Option<String>,
    // Optional fallback file used when the path is empty or resolves to a directory.
    // When None, missing/dir paths return 404.
    fallback: Option<PathBuf>,
}

impl ServeDir {
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self {
            root: root.into(),
            param: None,
            fallback: None,
        }
    }

    /// Specify which route parameter to read the relative file path from.
    /// Example: router.get("/assets/*p", Arc::new(ServeDir::new("assets").with_param_name("p")))
    pub fn with_param_name<S: Into<String>>(mut self, name: S) -> Self {
        self.param = Some(name.into());
        self
    }

    /// Set a fallback file name (e.g. "index.html").
    /// If set, it's used when the request path is empty or resolves to a directory.
    pub fn with_fallback<P: AsRef<str>>(mut self, name: P) -> Self {
        // sanitize the provided fallback path to avoid special components
        let mut out = PathBuf::new();
        for comp in Path::new(name.as_ref()).components() {
            if let Component::Normal(s) = comp {
                out.push(s);
            }
        }
        self.fallback = if out.as_os_str().is_empty() {
            None
        } else {
            Some(out)
        };
        self
    }

    fn sanitize(rel: &str) -> PathBuf {
        let mut out = PathBuf::new();
        for comp in Path::new(rel).components() {
            if let Component::Normal(s) = comp {
                out.push(s)
            }
        }
        out
    }
    /// Try to extract the relative path from the request in a flexible way:
    /// 1) Use explicitly configured param name when provided
    /// 2) Try common defaults: "path", "file"
    /// 3) If exactly one param exists, use it
    fn extract_rel_path<'a>(&self, req: &'a PingoraHttpRequest) -> Option<&'a str> {
        if let Some(name) = &self.param
            && let Some(v) = req.param(name)
            && !v.is_empty()
        {
            return Some(v);
        }
        if let Some(v) = req.param("path")
            && !v.is_empty()
        {
            return Some(v);
        }
        if let Some(v) = req.param("file")
            && !v.is_empty()
        {
            return Some(v);
        }
        // Fallback: single param case
        if req.params.len() == 1 {
            let (_, v) = req.params.iter().next().unwrap();
            if !v.is_empty() {
                return Some(v.as_str());
            }
        }
        None
    }
}

#[async_trait]
impl Handler for ServeDir {
    async fn handle(&self, req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        // Expect a param from pattern like "/assets/*path" or a configured param name.
        // If missing or empty (e.g., request "/assets"), use fallback when provided; else 404.
        let mut full = if let Some(rel) = self.extract_rel_path(&req) {
            let safe = Self::sanitize(rel);
            self.root.join(safe)
        } else if let Some(fb) = &self.fallback {
            self.root.join(fb)
        } else {
            return Ok(PingoraWebHttpResponse::text(
                StatusCode::NOT_FOUND,
                "Not Found",
            ));
        };

        // If the path is a directory, try appending index.html
        if let Ok(meta) = tokio::fs::metadata(&full).await
            && meta.is_dir()
        {
            if let Some(fb) = &self.fallback {
                full = full.join(fb);
            } else {
                return Ok(PingoraWebHttpResponse::text(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                ));
            }
        }

        // Canonicalize both root and the target to prevent escaping via symlinks
        let root_canon = match tokio::fs::canonicalize(&self.root).await {
            Ok(p) => p,
            Err(_) => {
                return Ok(PingoraWebHttpResponse::text(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                ));
            }
        };
        let full_canon = match tokio::fs::canonicalize(&full).await {
            Ok(p) => p,
            Err(_) => {
                return Ok(PingoraWebHttpResponse::text(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                ));
            }
        };

        // Enforce that the file must be within the root directory
        if !full_canon.starts_with(&root_canon) {
            return Ok(PingoraWebHttpResponse::text(
                StatusCode::NOT_FOUND,
                "Not Found",
            ));
        }

        match tokio::fs::metadata(&full_canon).await {
            Ok(meta) if meta.is_file() => Ok(PingoraWebHttpResponse::stream_file(
                StatusCode::OK,
                &full_canon,
            )),
            _ => Ok(PingoraWebHttpResponse::text(
                StatusCode::NOT_FOUND,
                "Not Found",
            )),
        }
    }
}

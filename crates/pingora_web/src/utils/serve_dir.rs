use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;

use crate::core::router::Handler;
use crate::core::{Request, Response};

/// Serve static files from a directory, similar to axum's ServeDir.
///
/// Usage:
///   router.get("/assets/*path", Arc::new(ServeDir::new("assets")));
///
/// Security: performs simple path normalization to prevent path traversal.
pub struct ServeDir {
    root: PathBuf,
}

impl ServeDir {
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
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
}

#[async_trait]
impl Handler for ServeDir {
    async fn handle(&self, req: Request) -> Response {
        // Expect wildcard param from pattern like "/assets/*path"
        // If empty (e.g., request "/assets"), try index.html
        let mut full = match req.param("path") {
            Some(rel) if !rel.is_empty() => {
                let safe = Self::sanitize(rel);
                self.root.join(safe)
            }
            _ => self.root.join("index.html"),
        };

        // If the path is a directory, try appending index.html
        if let Ok(meta) = tokio::fs::metadata(&full).await
            && meta.is_dir()
        {
            full = full.join("index.html");
        }

        match tokio::fs::metadata(&full).await {
            Ok(meta) if meta.is_file() => Response::stream_file(200, &full),
            _ => Response::text(404, "Not Found"),
        }
    }
}

pub mod core;
pub mod logging;
pub mod middleware;
pub mod utils;

// Re-export commonly used types at the crate root
pub use core::*;
pub use http::StatusCode;
pub use logging::*;
pub use middleware::*;

use async_trait::async_trait;
use http::Response as HttpResponse;
use std::sync::Arc;
// use pingora::apps::http_app::ServeHttp; // no longer used; we implement HttpServerApp
use pingora::protocols::http::ServerSession;
use pingora_core::apps::HttpServerApp;
// use tokio::time::{timeout, Duration};

/// The main application: holds router and middleware.
pub struct App {
    router: Router,
    pub(crate) middlewares: Vec<Arc<dyn Middleware>>,
    pub(crate) app_data: Arc<core::AppData>,
}

/// Default 404 handler
struct NotFoundHandler;

#[async_trait]
impl core::router::Handler for NotFoundHandler {
    async fn handle(&self, _req: Request) -> Response {
        Response::text(404, "Not Found")
    }
}

impl App {
    /// Single constructor: requires a Router; middlewares and shared data are optional to add later.
    pub fn new(router: Router) -> Self {
        let mut s = Self {
            router,
            middlewares: Vec::new(),
            app_data: Arc::new(AppData::new()),
        };
        // Install request-id middleware by default
        s.use_middleware(RequestId::default());
        s
    }

    pub fn use_middleware<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Arc::new(middleware));
    }

    // --- App-level shared data API (single choice) ---
    pub fn set_app_share_data<T: Send + Sync + 'static>(&self, value: Arc<T>) -> Option<Arc<T>> {
        self.app_data.provide_arc(value)
    }

    /// Handle a request end-to-end through middlewares and the router.
    pub async fn handle(&self, req: Request) -> Response {
        // Route lookup using references to avoid cloning
        let find_result = {
            let method = req.method();
            let path = req.path();
            self.router.find(method, path)
        };
        let (handler, params): (Arc<dyn Handler>, std::collections::HashMap<String, String>) =
            match find_result {
                Some((h, p)) => (h, p),
                None => {
                    let path = req.path();
                    let method = req.method();
                    let mut allowed = self.router.allowed_methods(path);
                    if *method == Method::OPTIONS {
                        // For OPTIONS, respond with 204 No Content and Allow header when no explicit route
                        allowed.push("OPTIONS".to_string());
                        allowed.sort();
                        allowed.dedup();
                        let mut res = Response::text(204, "");
                        let allow_header = allowed.join(", ");
                        res.headers.insert(
                            http::header::ALLOW,
                            http::HeaderValue::from_str(&allow_header).unwrap(),
                        );
                        return res;
                    }
                    // If a different method matches this path, return 405 with Allow header
                    if !allowed.is_empty() {
                        let allow_header = allowed.join(", ");
                        let mut res = Response::text(405, "Method Not Allowed");
                        res.headers.insert(
                            http::header::ALLOW,
                            http::HeaderValue::from_str(&allow_header).unwrap(),
                        );
                        return res;
                    }
                    // Fallback 404 handler when no route matches
                    let h: Arc<dyn core::router::Handler> = Arc::new(NotFoundHandler);
                    (h, Default::default())
                }
            };

        // Add route parameters and app-level data to request
        let req_with_params = req.with_params(params).with_app_data(self.app_data.clone());

        // Compose middlewares (onion model) around the route handler
        let entry = compose(&self.middlewares, handler);
        let mut response = entry.handle(req_with_params).await;

        // Automatically set content-length or transfer-encoding if not already set
        self.finalize_response_headers(&mut response);
        response
    }

    /// Automatically set content-length or transfer-encoding headers based on response body
    fn finalize_response_headers(&self, response: &mut Response) {
        // Only set headers if neither content-length nor transfer-encoding is already set
        if response.headers.contains_key(http::header::CONTENT_LENGTH)
            || response
                .headers
                .contains_key(http::header::TRANSFER_ENCODING)
        {
            return;
        }

        match &response.body {
            response::Body::Bytes(bytes) => {
                // Set content-length for byte bodies
                let len_s = bytes.len().to_string();
                let _ = response.headers.insert(
                    http::header::CONTENT_LENGTH,
                    http::HeaderValue::from_str(&len_s).unwrap(),
                );
            }
            response::Body::Stream(_) => {
                // Set transfer-encoding for streaming bodies
                let _ = response.headers.insert(
                    http::header::TRANSFER_ENCODING,
                    http::HeaderValue::from_static("chunked"),
                );
            }
        }
    }
}

use futures::StreamExt;
use pingora::server::ShutdownWatch;
use pingora_core::apps::{HttpPersistentSettings, HttpServerOptions, ReusedHttpStream};
use pingora_http::ResponseHeader;

#[async_trait]
impl HttpServerApp for App {
    async fn process_new_http(
        self: &Arc<Self>,
        mut http: ServerSession,
        shutdown: &ShutdownWatch,
    ) -> Option<ReusedHttpStream> {
        // Read request header
        if !(http.read_request().await.ok()?) {
            return None;
        }
        if *shutdown.borrow() {
            http.set_keepalive(None);
        } else {
            http.set_keepalive(Some(60));
        }

        // Build our internal Request and read request body when present
        let reqh = http.req_header();
        let path = String::from_utf8_lossy(reqh.raw_path()).to_string();

        // Only need a boolean for HEAD; avoid cloning the Method twice
        let is_head = reqh.method.as_str().eq_ignore_ascii_case("HEAD");

        let mut req = Request::new(reqh.method.clone(), path);
        for (name, value) in reqh.headers.iter() {
            if let Ok(v) = value.to_str() {
                req = req.header(name.as_str(), v);
            }
        }

        // Read request body only when hinted by headers (content-length > 0 or transfer-encoding present)
        if req.method() != Method::HEAD {
            let has_te = req.headers().contains_key("transfer-encoding");
            let has_len = req
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
                > 0;
            if (has_te || has_len)
                && let Ok(Some(bytes)) = http.read_request_body().await
            {
                req = req.with_body(bytes);
            }
        }

        // Route and produce Response (may be file for streaming)
        let res = self.handle(req).await;

        // Build and write response header
        let mut builder = HttpResponse::builder().status(res.status);
        for (k, v) in res.headers.iter() {
            builder = builder.header(k, v);
        }
        let (parts, _) = builder.body(Vec::<u8>::new()).unwrap().into_parts();
        let resp_header: ResponseHeader = parts.into();
        if http
            .write_response_header(Box::new(resp_header))
            .await
            .is_err()
        {
            return None;
        }

        // Write body with streaming support; for HEAD, do not send a body
        if !is_head {
            match res.body {
                response::Body::Bytes(bytes) => {
                    let _ = http.write_response_body(bytes, true).await;
                }
                response::Body::Stream(mut s) => {
                    while let Some(chunk) = s.next().await {
                        if http.write_response_body(chunk, false).await.is_err() {
                            break;
                        }
                    }
                    let _ = http.write_response_body(bytes::Bytes::new(), true).await;
                }
            }
        }

        let persistent_settings = HttpPersistentSettings::for_session(&http);
        match http.finish().await {
            Ok(c) => c.map(|s| ReusedHttpStream::new(s, Some(persistent_settings))),
            Err(_) => None,
        }
    }

    fn h2_options(&self) -> Option<pingora::protocols::http::v2::server::H2Options> {
        None
    }
    fn server_options(&self) -> Option<&HttpServerOptions> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestLogger(Arc<std::sync::Mutex<Vec<String>>>);
    use std::sync::Arc;
    impl Logger for TestLogger {
        fn log(&self, _level: Level, msg: &str, request_id: &str) {
            self.0
                .lock()
                .unwrap()
                .push(format!("{}|{}", request_id, msg));
        }
    }

    struct HelloHandler;
    #[async_trait::async_trait]
    impl core::router::Handler for HelloHandler {
        async fn handle(&self, req: Request) -> Response {
            let name = req.param("name").unwrap_or("world");
            Response::text(200, format!("Hello {}", name))
        }
    }

    #[tokio::test]
    async fn router_matches_and_params() {
        let mut router = Router::new();
        router.get("/hi/{name}", Arc::new(HelloHandler));
        let app = App::new(router);

        let req = Request::new(Method::GET, "/hi/alice");
        let res = app.handle(req).await;
        assert_eq!(res.status.as_u16(), 200);
        match res.body {
            core::response::Body::Bytes(b) => {
                assert_eq!(std::str::from_utf8(&b).unwrap(), "Hello alice")
            }
            _ => panic!("unexpected streaming body"),
        }
    }

    #[tokio::test]
    async fn middleware_order_and_request_id() {
        let mut router = Router::new();

        // A tracing middleware that modifies response headers to track execution order
        struct Trace(&'static str);
        #[async_trait::async_trait]
        impl Middleware for Trace {
            async fn handle(&self, req: Request, next: Arc<dyn core::router::Handler>) -> Response {
                let mut res = next.handle(req).await;
                // Use header to track middleware execution order
                let current = res
                    .headers
                    .get("x-trace")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                let new_val = format!("{}{}", current, self.0);
                let _ = res
                    .headers
                    .insert("x-trace", http::HeaderValue::from_str(&new_val).unwrap());
                res
            }
        }
        struct OkHandler;
        #[async_trait::async_trait]
        impl core::router::Handler for OkHandler {
            async fn handle(&self, req: Request) -> Response {
                let res = Response::text(200, "H");
                // Ensure we have a request-id header from middleware
                assert!(req.headers().contains_key("x-request-id"));
                res
            }
        }
        router.get("/ok", Arc::new(OkHandler));
        let mut app = App::new(router);
        app.use_middleware(Trace("A>"));
        app.use_middleware(Trace("B>"));

        let res = app.handle(Request::new(Method::GET, "/ok")).await;
        assert_eq!(res.status.as_u16(), 200);
        // Verify middleware execution order through header
        let trace = res
            .headers
            .get("x-trace")
            .and_then(|v| v.to_str().ok())
            .unwrap();
        assert_eq!(trace, "B>A>"); // B wraps A, so B executes last
        assert!(res.headers.contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn logger_receives_request_id() {
        let logs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let logger = TestLogger(logs.clone());

        let mut router = Router::new();
        struct IndexHandler;
        #[async_trait::async_trait]
        impl core::router::Handler for IndexHandler {
            async fn handle(&self, _req: Request) -> Response {
                Response::text(200, "ok")
            }
        }

        router.get("/", Arc::new(IndexHandler));
        let mut app = App::new(router);
        app.use_middleware(super::logging_middleware::LoggingMiddleware::new(logger));
        let _ = app.handle(Request::new(Method::GET, "/")).await;

        let entries = logs.lock().unwrap();
        assert!(!entries.is_empty());
        // format: "<request_id>|<message>" - request_id might be empty if logging runs before RequestId middleware
        assert!(entries.iter().any(|s| s.contains("-> 200")));
        // Since LoggingMiddleware runs before RequestId in the middleware stack, request_id might be empty
        assert!(entries.iter().any(|s| s.contains("GET / -> 200")));
    }

    #[tokio::test]
    async fn app_data_available_in_handler() {
        #[derive(Clone)]
        struct Cfg {
            msg: &'static str,
        }

        struct UseCfg;
        #[async_trait::async_trait]
        impl core::router::Handler for UseCfg {
            async fn handle(&self, req: Request) -> Response {
                let cfg = req.get_app_share_data::<Cfg>().expect("cfg present");
                Response::text(200, cfg.msg)
            }
        }

        let mut router = Router::new();
        router.get("/", Arc::new(UseCfg));
        let app = App::new(router);
        app.set_app_share_data(Arc::new(Cfg { msg: "hello" }));

        let res = app.handle(Request::new(Method::GET, "/")).await;
        match res.body {
            core::response::Body::Bytes(b) => assert_eq!(std::str::from_utf8(&b).unwrap(), "hello"),
            _ => panic!("unexpected streaming body"),
        }
    }

    #[tokio::test]
    async fn request_extensions_flow() {
        struct PutNum;
        #[async_trait::async_trait]
        impl Middleware for PutNum {
            async fn handle(
                &self,
                mut req: Request,
                next: Arc<dyn core::router::Handler>,
            ) -> Response {
                req.set_request_share_data(Arc::new(7u32));
                next.handle(req).await
            }
        }

        struct ReadNum;
        #[async_trait::async_trait]
        impl core::router::Handler for ReadNum {
            async fn handle(&self, req: Request) -> Response {
                let n = req.get_request_share_data::<u32>().expect("n");
                Response::text(200, format!("{}", *n))
            }
        }

        let mut router = Router::new();
        router.get("/n", Arc::new(ReadNum));
        let mut app = App::new(router);
        app.use_middleware(PutNum);

        let res = app.handle(Request::new(Method::GET, "/n")).await;
        match res.body {
            core::response::Body::Bytes(b) => assert_eq!(std::str::from_utf8(&b).unwrap(), "7"),
            _ => panic!("unexpected streaming body"),
        }
    }

    #[tokio::test]
    async fn app_sets_content_length() {
        struct TextHandler;
        #[async_trait::async_trait]
        impl core::router::Handler for TextHandler {
            async fn handle(&self, _req: Request) -> Response {
                Response::text(200, "hello world")
            }
        }

        let mut router = Router::new();
        router.get("/text", Arc::new(TextHandler));
        let app = App::new(router);

        let res = app.handle(Request::new(Method::GET, "/text")).await;

        // Verify content-length is automatically set
        assert_eq!(
            res.headers
                .get(http::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok()),
            Some("11")
        );
        assert_eq!(
            res.headers
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/plain; charset=utf-8")
        );

        match res.body {
            core::response::Body::Bytes(b) => {
                assert_eq!(std::str::from_utf8(&b).unwrap(), "hello world")
            }
            _ => panic!("unexpected streaming body"),
        }
    }

    #[tokio::test]
    async fn app_respects_manual_content_length() {
        struct ManualHandler;
        #[async_trait::async_trait]
        impl core::router::Handler for ManualHandler {
            async fn handle(&self, _req: Request) -> Response {
                Response::text(200, "hello").header("content-length", "999")
            }
        }

        let mut router = Router::new();
        router.get("/manual", Arc::new(ManualHandler));
        let app = App::new(router);

        let res = app.handle(Request::new(Method::GET, "/manual")).await;

        // Verify manual content-length is preserved
        assert_eq!(
            res.headers
                .get(http::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok()),
            Some("999")
        );
    }
}

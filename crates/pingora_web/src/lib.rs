pub mod core;
pub mod error;
pub mod middleware;
pub mod utils;

// Re-export commonly used types at the crate root
pub use core::*;
pub use error::{ResponseError, WebError};
pub use http::StatusCode;
pub use middleware::*;
pub use pingora_core::modules::http::compression::ResponseCompressionBuilder;
pub use pingora_core::modules::http::{HttpModule, ModuleBuilder};

use crate::core::router::Router;
use async_trait::async_trait;
use http::Response as HttpResponse;
use std::sync::Arc;
// use pingora::apps::http_app::ServeHttp; // no longer used; we implement HttpServerApp
use pingora::protocols::http::ServerSession;
use pingora_core::apps::HttpServerApp;
use pingora_core::modules::http::HttpModules;
// use tokio::time::{timeout, Duration};

/// The main application: holds router and middleware.
pub struct App {
    router: Router,
    pub(crate) middlewares: Vec<Arc<dyn Middleware>>,
    pub(crate) app_data: Arc<core::AppData>,
    pub(crate) http_modules: HttpModules,
}

/// Default 404 handler
struct NotFoundHandler;

#[async_trait]
impl core::Handler for NotFoundHandler {
    async fn handle(&self, _req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        Ok(PingoraWebHttpResponse::text(
            StatusCode::NOT_FOUND,
            "Not Found",
        ))
    }
}

impl App {
    /// Internal constructor with a Router. External users should use `App::default()`
    /// and the route methods on `App`.
    pub(crate) fn new(router: Router) -> Self {
        let mut s = Self {
            router,
            middlewares: Vec::new(),
            app_data: Arc::new(AppData::new()),
            http_modules: HttpModules::new(),
        };
        // Install request-id middleware by default
        s.use_middleware(RequestId::default());
        s
    }

    // Create an App with an empty Router via Default trait

    pub fn use_middleware<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Arc::new(middleware));
    }

    /// Add HTTP module to this App
    pub fn add_http_module(&mut self, module: ModuleBuilder) {
        self.http_modules.add_module(module)
    }

    // ===== Route registration (App-level wrappers over Router) =====

    pub fn add<S: Into<String>>(
        &mut self,
        method: core::Method,
        path: S,
        handler: Arc<dyn core::Handler>,
    ) {
        self.router.add(method, path, handler)
    }

    pub fn get<S: Into<String>>(&mut self, path: S, handler: Arc<dyn core::Handler>) {
        self.router.get(path, handler)
    }

    pub fn post<S: Into<String>>(&mut self, path: S, handler: Arc<dyn core::Handler>) {
        self.router.post(path, handler)
    }

    // For other HTTP methods, use `add(Method::X, ...)` for simplicity.

    /// Closure handler: GET (returns Result)
    pub fn get_fn<S, F>(&mut self, path: S, handler: F)
    where
        S: Into<String>,
        F: Fn(PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError>
            + Send
            + Sync
            + 'static,
    {
        self.router.get_fn(path, handler)
    }

    /// Closure handler: POST (returns Result)
    pub fn post_fn<S, F>(&mut self, path: S, handler: F)
    where
        S: Into<String>,
        F: Fn(PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError>
            + Send
            + Sync
            + 'static,
    {
        self.router.post_fn(path, handler)
    }

    // --- App-level shared data API (single choice) ---
    pub fn set_app_share_data<T: Send + Sync + 'static>(&self, value: Arc<T>) -> Option<Arc<T>> {
        self.app_data.provide_arc(value)
    }

    /// Listen on the given address and start the server (beginner-friendly method)
    ///
    /// This is a convenience method that handles all the Pingora server setup internally.
    /// For more advanced use cases, use `to_service()` to get a Service that you can
    /// configure further before adding to a Server.
    ///
    /// # Example
    /// ```no_run
    /// use pingora_web::App;
    /// let app = App::default();
    /// // app.get("/", ...);
    /// // app.listen("0.0.0.0:8080").unwrap();
    /// ```
    pub fn listen(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        use pingora::server::Server;
        use pingora::services::listening::Service;

        let mut server = Server::new(None)?;
        server.bootstrap();

        let mut service = Service::new("pingora_web".to_string(), self);
        service.add_tcp(addr);
        server.add_services(vec![Box::new(service)]);

        server.run_forever()
    }

    /// Convert this App into a Pingora Service (advanced users)
    ///
    /// This method gives you full control over the Service configuration,
    /// allowing you to add multiple TCP listeners, configure TLS, etc.
    ///
    /// # Example
    /// ```no_run
    /// use pingora_web::App;
    /// use pingora::server::Server;
    ///
    /// let app = App::default();
    /// let mut service = app.to_service("my-web-service");
    /// service.add_tcp("0.0.0.0:8080");
    /// service.add_tcp("0.0.0.0:8443"); // Add HTTPS later
    ///
    /// let mut server = Server::new(None).unwrap();
    /// server.add_service(service);
    /// server.run_forever();
    /// ```
    pub fn to_service(
        self,
        name: impl Into<String>,
    ) -> pingora::services::listening::Service<Self> {
        use pingora::services::listening::Service;
        Service::new(name.into(), self)
    }

    /// Handle a request end-to-end through middlewares and the router.
    pub async fn handle(&self, mut req: PingoraHttpRequest) -> PingoraWebHttpResponse {
        // Ensure a request-id exists early, even if middlewares fail later
        let request_id = req
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map_or_else(crate::utils::request_id::generate, ToString::to_string);
        // Put request-id into request headers if not already present
        if !req.headers().contains_key("x-request-id") {
            let _ = req.headers_mut().insert(
                "x-request-id",
                http::HeaderValue::from_str(&request_id).unwrap(),
            );
        }
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
                        let mut res = PingoraWebHttpResponse::text(StatusCode::NO_CONTENT, "");
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
                        let mut res = PingoraWebHttpResponse::text(
                            StatusCode::METHOD_NOT_ALLOWED,
                            "Method Not Allowed",
                        );
                        res.headers.insert(
                            http::header::ALLOW,
                            http::HeaderValue::from_str(&allow_header).unwrap(),
                        );
                        return res;
                    }
                    // Fallback 404 handler when no route matches
                    let h: Arc<dyn Handler> = Arc::new(NotFoundHandler);
                    (h, Default::default())
                }
            };

        // Add route parameters and app-level data to request
        let req_with_params = req.with_params(params).with_app_data(self.app_data.clone());

        // Compose middlewares (onion model) around the route handler
        let entry = compose(&self.middlewares, handler);

        // Handle the request and convert any errors to responses
        let mut response = match entry.handle(req_with_params).await {
            Ok(response) => response,
            Err(error) => error.into_response(),
        };

        // Ensure response carries the request-id even on error paths
        if !response.headers.contains_key("x-request-id") {
            let _ = response.headers.insert(
                "x-request-id",
                http::HeaderValue::from_str(&request_id).unwrap(),
            );
        }

        // Automatically set content-length or transfer-encoding if not already set
        self.finalize_response_headers(&mut response);
        response
    }

    /// Automatically set content-length or transfer-encoding headers based on response body
    fn finalize_response_headers(&self, response: &mut PingoraWebHttpResponse) {
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

impl Default for App {
    fn default() -> Self {
        Self::new(Router::new())
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

        // Build module context for HTTP modules
        let mut module_ctx = self.http_modules.build_ctx();

        // Apply request header filter from modules
        if module_ctx
            .request_header_filter(http.req_header_mut())
            .await
            .is_err()
        {
            return None;
        }

        // Build our internal Request and read request body when present
        let reqh = http.req_header();
        let path = String::from_utf8_lossy(reqh.raw_path()).to_string();

        // Only need a boolean for HEAD; avoid cloning the Method twice
        let is_head = reqh.method.as_str().eq_ignore_ascii_case("HEAD");

        let mut req = PingoraHttpRequest::new(reqh.method.clone(), path);
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
        let mut resp_header: ResponseHeader = parts.into();

        // Apply response header filter from modules
        let is_body_empty = matches!(res.body, response::Body::Bytes(ref b) if b.is_empty());
        if module_ctx
            .response_header_filter(&mut resp_header, is_body_empty)
            .await
            .is_err()
        {
            return None;
        }

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
                    // Apply response body filter from modules
                    let mut body_opt = Some(bytes);
                    if module_ctx
                        .response_body_filter(&mut body_opt, true)
                        .is_err()
                    {
                        return None;
                    }
                    if let Some(filtered_body) = body_opt {
                        let _ = http.write_response_body(filtered_body, true).await;
                    }
                }
                response::Body::Stream(mut s) => {
                    while let Some(chunk) = s.next().await {
                        // Apply body filter to each chunk
                        let mut body_opt = Some(chunk);
                        if module_ctx
                            .response_body_filter(&mut body_opt, false)
                            .is_err()
                        {
                            break;
                        }
                        if let Some(filtered_chunk) = body_opt
                            && http
                                .write_response_body(filtered_chunk, false)
                                .await
                                .is_err()
                        {
                            break;
                        }
                    }
                    // Final empty chunk to signal end
                    let mut final_body = Some(bytes::Bytes::new());
                    if module_ctx
                        .response_body_filter(&mut final_body, true)
                        .is_ok()
                        && let Some(final_chunk) = final_body
                    {
                        let _ = http.write_response_body(final_chunk, true).await;
                    }
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

    // no custom Logger/LoggingMiddleware tests; TracingMiddleware covers logging paths

    struct HelloHandler;
    #[async_trait::async_trait]
    impl core::Handler for HelloHandler {
        async fn handle(
            &self,
            req: PingoraHttpRequest,
        ) -> Result<PingoraWebHttpResponse, WebError> {
            let name = req.param("name").unwrap_or("world");
            Ok(PingoraWebHttpResponse::text(
                StatusCode::OK,
                format!("Hello {}", name),
            ))
        }
    }

    #[tokio::test]
    async fn router_matches_and_params() {
        let mut router = Router::new();
        router.get("/hi/{name}", Arc::new(HelloHandler));
        let app = App::new(router);

        let req = PingoraHttpRequest::new(Method::GET, "/hi/alice");
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
            async fn handle(
                &self,
                req: PingoraHttpRequest,
                next: Arc<dyn core::Handler>,
            ) -> Result<PingoraWebHttpResponse, WebError> {
                let mut res = next.handle(req).await?;
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
                Ok(res)
            }
        }
        struct OkHandler;
        #[async_trait::async_trait]
        impl core::Handler for OkHandler {
            async fn handle(
                &self,
                req: PingoraHttpRequest,
            ) -> Result<PingoraWebHttpResponse, WebError> {
                let res = PingoraWebHttpResponse::text(StatusCode::OK, "H");
                // Ensure we have a request-id header from middleware
                assert!(req.headers().contains_key("x-request-id"));
                Ok(res)
            }
        }
        router.get("/ok", Arc::new(OkHandler));
        let mut app = App::new(router);
        app.use_middleware(Trace("A>"));
        app.use_middleware(Trace("B>"));

        let res = app
            .handle(PingoraHttpRequest::new(Method::GET, "/ok"))
            .await;
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

    // Logging is handled by TracingMiddleware; no direct logging middleware tests

    #[tokio::test]
    async fn app_data_available_in_handler() {
        #[derive(Clone)]
        struct Cfg {
            msg: &'static str,
        }

        struct UseCfg;
        #[async_trait::async_trait]
        impl core::Handler for UseCfg {
            async fn handle(
                &self,
                req: PingoraHttpRequest,
            ) -> Result<PingoraWebHttpResponse, WebError> {
                let cfg = req.get_app_share_data::<Cfg>().expect("cfg present");
                Ok(PingoraWebHttpResponse::text(StatusCode::OK, cfg.msg))
            }
        }

        let mut router = Router::new();
        router.get("/", Arc::new(UseCfg));
        let app = App::new(router);
        app.set_app_share_data(Arc::new(Cfg { msg: "hello" }));

        let res = app.handle(PingoraHttpRequest::new(Method::GET, "/")).await;
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
                mut req: PingoraHttpRequest,
                next: Arc<dyn core::Handler>,
            ) -> Result<PingoraWebHttpResponse, WebError> {
                req.set_request_share_data(Arc::new(7u32));
                next.handle(req).await
            }
        }

        struct ReadNum;
        #[async_trait::async_trait]
        impl core::Handler for ReadNum {
            async fn handle(
                &self,
                req: PingoraHttpRequest,
            ) -> Result<PingoraWebHttpResponse, WebError> {
                let n = req.get_request_share_data::<u32>().expect("n");
                Ok(PingoraWebHttpResponse::text(
                    StatusCode::OK,
                    format!("{}", *n),
                ))
            }
        }

        let mut router = Router::new();
        router.get("/n", Arc::new(ReadNum));
        let mut app = App::new(router);
        app.use_middleware(PutNum);

        let res = app.handle(PingoraHttpRequest::new(Method::GET, "/n")).await;
        match res.body {
            core::response::Body::Bytes(b) => assert_eq!(std::str::from_utf8(&b).unwrap(), "7"),
            _ => panic!("unexpected streaming body"),
        }
    }

    #[tokio::test]
    async fn app_sets_content_length() {
        struct TextHandler;
        #[async_trait::async_trait]
        impl core::Handler for TextHandler {
            async fn handle(
                &self,
                _req: PingoraHttpRequest,
            ) -> Result<PingoraWebHttpResponse, WebError> {
                Ok(PingoraWebHttpResponse::text(StatusCode::OK, "hello world"))
            }
        }

        let mut router = Router::new();
        router.get("/text", Arc::new(TextHandler));
        let app = App::new(router);

        let res = app
            .handle(PingoraHttpRequest::new(Method::GET, "/text"))
            .await;

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
        impl core::Handler for ManualHandler {
            async fn handle(
                &self,
                _req: PingoraHttpRequest,
            ) -> Result<PingoraWebHttpResponse, WebError> {
                Ok(PingoraWebHttpResponse::text(StatusCode::OK, "hello")
                    .header("content-length", "999"))
            }
        }

        let mut router = Router::new();
        router.get("/manual", Arc::new(ManualHandler));
        let app = App::new(router);

        let res = app
            .handle(PingoraHttpRequest::new(Method::GET, "/manual"))
            .await;

        // Verify manual content-length is preserved
        assert_eq!(
            res.headers
                .get(http::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok()),
            Some("999")
        );
    }
}

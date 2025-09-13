# Pingora Web

Minimal routing, middleware, and structured logging (with request ID) for servers built on Cloudflare's Pingora. This workspace contains the library crate and a runnable example that starts a Pingora HTTP server using the library.

## Features

- Simple, ergonomic router with parameter support (e.g. `/users/{id}`)
- Async middleware in an onion model; easy composition
- Request ID middleware enabled by default (`x-request-id` header)
- Tracing-friendly logging middleware and types
- App implements Pingora `ServeHttp` for direct integration

## Workspace Layout

- Library: `crates/pingora_web`
- Example server: `examples/pingora_example`

## Quick Start (run the example)

```bash
cargo run -p pingora_example
```

This starts an HTTP server on `0.0.0.0:8080` with a few routes:

- `GET /` → `ok`
- `GET /foo` → `get_foo`
- `GET /foo/bar` → `foo_bar`

## Usage (library)

Add the dependency if using from another workspace:

```toml
[dependencies]
pingora_web = { path = "./crates/pingora_web" }
pingora = { version = "0.6", features = ["lb"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### Define handlers and routes

```rust
use async_trait::async_trait;
use pingora_web::{App, Handler, Request, Response, Router, TracingMiddleware};
use pingora::apps::http_app::HttpServer;
use pingora::server::Server;
use pingora::services::listening::Service;
use std::sync::Arc;

struct RootHandler;

#[async_trait]
impl Handler for RootHandler {
    async fn handle(&self, _req: Request) -> Response {
        Response::text(200, "ok")
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let mut router = Router::new();
    router.get("/", Arc::new(RootHandler));

    let mut app = App::new(router);
    // RequestId middleware is added by default; add tracing if desired
    app.use_middleware(TracingMiddleware::new());

    // Pingora integration
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    let mut service = Service::new(
        "Web Service HTTP".to_string(),
        HttpServer::new_app(app),
    );
    service.add_tcp("0.0.0.0:8080");
    server.add_services(vec![Box::new(service)]);
    server.run_forever();
}
```

### Route parameters

```rust
use async_trait::async_trait;
use pingora_web::{Handler, Request, Response, Router};
use std::sync::Arc;

struct HelloHandler;

#[async_trait]
impl Handler for HelloHandler {
    async fn handle(&self, req: Request) -> Response {
        let name = req.param("name").unwrap_or("world");
        Response::text(200, format!("Hello {}", name))
    }
}

let mut router = Router::new();
router.get("/hi/{name}", Arc::new(HelloHandler));
```

## Middleware

- Built-ins:
  - `RequestId` (installed by default in `App::new()`)
  - `TracingMiddleware` (structured logs with request scope)
  - `LoggingMiddleware` (pluggable logger: `StdoutLogger` or your own via `Logger` trait)

Custom middleware implements the `Middleware` trait and calls `next` to continue the chain:

```rust
use async_trait::async_trait;
use pingora_web::{App, Handler, Middleware, Request, Response};
use std::sync::Arc;

struct AuthMiddleware;

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn handle(&self, req: Request, next: Arc<dyn Handler>) -> Response {
        if req.headers.get("authorization").is_none() {
            return Response::text(401, "Unauthorized");
        }
        next.handle(req).await
    }
}

let mut router = Router::new();
let mut app = App::new(router);
app.use_middleware(AuthMiddleware);
```

Middleware composition uses an onion model: the last registered runs first on the way in and last on the way out.

## Request & Response

- `Request`
  - Fields: `method`, `path`, `headers`, `body`
  - Helpers: `param(name)`, `param_or(name, default)`, builder-style `header()` and `with_body()`
- `Response`
  - Constructors: `new(status)`, `text(status, body)`
  - Headers: `set_header()`, builder-style `header()`

## Shared Data

- Two levels:
  - App-level: set once on `App`, read within any request via `Request`.
  - Request-level: set during a request (middleware/handler), read later in the same request.

Example:

```rust
use std::sync::Arc;
use std::time::Instant;

// App-level config
#[derive(Clone)]
struct Config { name: &'static str }

let mut router = Router::new();
let mut app = App::new(router);
app.set_app_share_data(Arc::new(Config { name: "pingora_web" }));

// Request-level: set in middleware
struct Timer;
#[async_trait::async_trait]
impl Middleware for Timer {
    async fn handle(&self, mut req: Request, next: Arc<dyn Handler>) -> Response {
        req.set_request_share_data(Arc::new(Instant::now()));
        next.handle(req).await
    }
}
app.use_middleware(Timer);

// Read in handler
struct Show;
#[async_trait::async_trait]
impl Handler for Show {
    async fn handle(&self, req: Request) -> Response {
        let cfg = req.get_app_share_data::<Config>().unwrap();
        let start = req.get_request_share_data::<Instant>().unwrap();
        Response::text(200, format!("{} in {}ms", cfg.name, start.elapsed().as_millis()))
    }
}
router.get("/stats", Arc::new(Show));
```

## Development

- Build/check: `cargo check`, `cargo build`
- Tests: `cargo test`
- Format: `cargo fmt`
- Lint: `cargo clippy -- -D warnings`

## License

Dual-licensed at your option:

- MIT
- Apache-2.0

## Acknowledgments

- Built on [Pingora](https://github.com/cloudflare/pingora)
- Routing powered by [matchit](https://github.com/ibraheemdev/matchit)

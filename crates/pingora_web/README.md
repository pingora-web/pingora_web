# pingora_web

Minimal routing, middleware, and structured logging (with request ID) for servers built on Cloudflare's Pingora.

- Router with params (e.g. `/hi/{name}`)
- Async middleware (onion model)
- Request ID middleware enabled by default (`x-request-id`)
- Tracing-friendly logging middleware
- Integrates with Pingora as an `HttpServerApp`

## Installation

```toml
[dependencies]
pingora_web = "0.1"
pingora = { version = "0.6", features = ["lb"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

## Quick example

```rust
use async_trait::async_trait;
use pingora_web::{App, Handler, Request, Response, Router, TracingMiddleware};
use pingora::server::Server;
use pingora::services::listening::Service;
use std::sync::Arc;

struct Hello;
#[async_trait]
impl Handler for Hello {
    async fn handle(&self, req: Request) -> Response {
        let name = req.param("name").unwrap_or("world");
        Response::text(200, format!("Hello {}", name))
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let mut router = Router::new();
    router.get("/hi/{name}", Arc::new(Hello));

    let mut app = App::new(router);
    app.use_middleware(TracingMiddleware::new());

    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    let mut service = Service::new("Web Service HTTP".to_string(), app);
    service.add_tcp("0.0.0.0:8080");
    server.add_services(vec![Box::new(service)]);
    server.run_forever().unwrap();
}
```

## License

Dual-licensed under either:
- MIT
- Apache-2.0

at your option.

Source repository: https://github.com/zaijie1213/pingora_web
Documentation: https://docs.rs/pingora_web

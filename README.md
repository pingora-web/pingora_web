# pingora_web

[![CI](https://github.com/zaijie1213/pingora_web/actions/workflows/ci.yml/badge.svg)](https://github.com/zaijie1213/pingora_web/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/pingora_web.svg)](https://crates.io/crates/pingora_web)
[![Documentation](https://docs.rs/pingora_web/badge.svg)](https://docs.rs/pingora_web)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

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
pingora = { version = "0.6" }
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

## JSON Response Example

```rust
use serde::Serialize;

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
    data: Vec<String>,
}

struct JsonHandler;
#[async_trait]
impl Handler for JsonHandler {
    async fn handle(&self, _req: Request) -> Response {
        let response = ApiResponse {
            success: true,
            message: "Hello from JSON API".to_string(),
            data: vec!["item1".to_string(), "item2".to_string()],
        };
        Response::json(200, response)
    }
}

// Add to router:
// router.get("/api/data", Arc::new(JsonHandler));
```

## Static File Serving Example

```rust
use pingora_web::utils::ServeDir;

fn setup_router() -> Router {
    let mut router = Router::new();

    // Serve static files from ./public directory
    router.get("/static/{path}", Arc::new(ServeDir::new("./public")));

    // Or serve from current directory
    router.get("/assets/{path}", Arc::new(ServeDir::new(".")));

    router
}
```

## Development

### Prerequisites

- Rust 1.75 or later
- Git

### Building

```bash
git clone https://github.com/zaijie1213/pingora_web.git
cd pingora_web
cargo build
```

### Testing

```bash
cargo test
```

### Code Quality

This project uses several tools to maintain code quality:

```bash
# Format code
cargo fmt

# Lint code
cargo clippy --all-targets --all-features -- -D warnings

# Security audit
cargo audit
```

### Running Examples

```bash
cargo run --example pingora_example
```

Then visit:
- `http://localhost:8080/` - Basic response
- `http://localhost:8080/foo` - Static route
- `http://localhost:8080/hi/yourname` - Route with parameters
- `http://localhost:8080/json` - JSON response
- `http://localhost:8080/assets/README.md` - Static file serving

## Release Process

This project uses automated releases through GitHub Actions:

1. **Create a new tag**: `git tag v0.1.1 && git push origin v0.1.1`
2. **GitHub Actions will**:
   - Run all tests and quality checks
   - Create a GitHub release with auto-generated notes
   - Publish the crate to [crates.io](https://crates.io)
   - Verify the publication

### Setting Up Automated Releases

To enable automated publishing to crates.io:

1. Get your API token from [crates.io/me](https://crates.io/me)
2. Add it as a repository secret named `CARGO_REGISTRY_TOKEN`
3. Push a version tag to trigger the release workflow

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and quality checks
5. Submit a pull request

All pull requests are automatically tested with GitHub Actions.

## License

Dual-licensed under either:
- MIT
- Apache-2.0

at your option.

Source repository: https://github.com/zaijie1213/pingora_web
Documentation: https://docs.rs/pingora_web
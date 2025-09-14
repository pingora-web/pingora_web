# 🚀 pingora_web

[![CI](https://github.com/pingora-web/pingora_web/actions/workflows/ci.yml/badge.svg)](https://github.com/pingora-web/pingora_web/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/pingora_web.svg)](https://crates.io/crates/pingora_web)
[![Documentation](https://docs.rs/pingora_web/badge.svg)](https://docs.rs/pingora_web)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/pingora_web.svg)](https://crates.io/crates/pingora_web)
[![Stars](https://img.shields.io/github/stars/pingora-web/pingora_web.svg)](https://github.com/pingora-web/pingora_web)

**🔥 Fast setup | Built on Pingora | Beginner friendly** 🦀

[English](README.md) | [中文](README_zh.md)

A web framework built on Cloudflare's Pingora proxy infrastructure, designed to be fast, reliable, and easy to use.

## ✨ Features

### Core Features
- 🛣️ **Path routing** with parameters (`/users/{id}`)
- 🧅 **Middleware system** with onion model (like Express.js)
- 🏷️ **Request ID tracking** (automatic `x-request-id` header)
- 📝 **Structured logging** with tracing integration
- 📦 **JSON support** with automatic serialization
- 📁 **Static file serving** with MIME type detection
- 🌊 **Streaming responses** for large data transfers

### Built on Pingora
- ⚡ **High performance** - leverages Cloudflare's production-tested proxy
- 🗜️ **HTTP compression** - built-in gzip support
- 🛡️ **Request limits** - timeout, body size, and header constraints
- 🚨 **Panic recovery** - automatic error handling
- 🔗 **HTTP/1.1 & HTTP/2** support via Pingora

## 🚀 Quick Start

### 1. Create a new project
```bash
cargo new my_api && cd my_api
```

### 2. Add dependencies to `Cargo.toml`

**Minimal setup (Hello World):**
```toml
[dependencies]
pingora_web = "0.1"
```

**Full setup (with JSON, logging, etc.):**
```toml
[dependencies]
pingora_web = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### 3. Hello World (5 lines - like Express/Gin)

```rust
use pingora_web::{App, StatusCode, PingoraWebHttpResponse, WebError, PingoraHttpRequest};

fn main() {
    let mut app = App::default();
    app.get_fn("/", |_req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, "Hello World!"))
    });
    app.listen("0.0.0.0:8080").unwrap();
}
```

### 4. With Parameters (beginner-friendly)

```rust
use pingora_web::{App, StatusCode, PingoraWebHttpResponse, WebError, PingoraHttpRequest};

fn main() {
    let mut app = App::default();
    app.get_fn("/", |_req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, "Hello World!"))
    });
    app.get_fn("/hi/{name}", |req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        let name = req.param("name").unwrap_or("world");
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, format!("Hello {}", name)))
    });
    app.listen("0.0.0.0:8080").unwrap();
}
```

### 5. Full-featured example

```rust
use async_trait::async_trait;
use pingora_web::{App, Handler, StatusCode, TracingMiddleware, ResponseCompressionBuilder, WebError, PingoraHttpRequest, PingoraWebHttpResponse};
use std::sync::Arc;

struct Hello;
#[async_trait]
impl Handler for Hello {
    async fn handle(&self, req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        let name = req.param("name").unwrap_or("world");
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, format!("Hello {}", name)))
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let mut app = App::default();
    app.get("/hi/{name}", Arc::new(Hello));
    app.use_middleware(TracingMiddleware::new());
    app.add_http_module(ResponseCompressionBuilder::enable(6));

    app.listen("0.0.0.0:8080").unwrap();
}
```

### 6. Run the server
```bash
cargo run
```

Visit `http://localhost:8080/` or `http://localhost:8080/hi/world` to see it working!

### Advanced usage (for complex setups)

If you need more control over the server configuration:

```rust
use async_trait::async_trait;
use pingora_web::{App, Handler, StatusCode, WebError, PingoraHttpRequest, PingoraWebHttpResponse};
use pingora::server::Server;
use std::sync::Arc;

struct Hello;
#[async_trait]
impl Handler for Hello {
    async fn handle(&self, req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        let name = req.param("name").unwrap_or("world");
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, format!("Hello {}", name)))
    }
}

fn main() {
    let mut app = App::default();
    app.get("/hi/{name}", Arc::new(Hello));
    let app = app;

    // Advanced: Convert to service for more control
    let mut service = app.to_service("my-web-app");
    service.add_tcp("0.0.0.0:8080");
    service.add_tcp("[::]:8080"); // IPv6 support

    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    server.add_service(service);

    // Add monitoring endpoint
    let mut prometheus_service = pingora::services::listening::Service::prometheus_http_service();
    prometheus_service.add_tcp("127.0.0.1:9090");
    server.add_service(prometheus_service);

    server.run_forever();
}
```

## 📖 Documentation

- **[Architecture & Design Philosophy](ARCHITECTURE.md)** - Detailed explanation of design decisions and architectural patterns

## 📚 Examples

### JSON API (核心功能)

Building REST APIs is the primary use case:

```rust
use pingora_web::{App, StatusCode, PingoraWebHttpResponse, WebError, PingoraHttpRequest};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

#[derive(Deserialize)]
struct CreateUser {
    name: String,
    email: String,
}

fn main() {
    let mut app = App::default();
    // GET /users/{id}
    app.get_fn("/users/{id}", |req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        let user_id: u64 = req.param("id").unwrap_or("0").parse().unwrap_or(0);
        let user = User {
            id: user_id,
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        };
        Ok(PingoraWebHttpResponse::json(StatusCode::OK, user))
    });

    // POST /users
    app.post_fn("/users", |req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        match serde_json::from_slice::<CreateUser>(req.body()) {
            Ok(create_user) => {
                let user = User {
                    id: 123,
                    name: create_user.name,
                    email: create_user.email,
                };
                Ok(PingoraWebHttpResponse::json(StatusCode::CREATED, user))
            }
            Err(_) => Ok(PingoraWebHttpResponse::json(StatusCode::BAD_REQUEST, serde_json::json!({
                "error": "Invalid JSON"
            })))
        }
    });

    app.listen("0.0.0.0:8080").unwrap();
}
```

## HTTP Modules (Built-in Pingora Features)

pingora_web integrates Pingora's high-performance HTTP modules for advanced functionality:

```rust
use pingora_web::{App, ResponseCompressionBuilder};

fn main() {
    let mut app = App::default();
    // ... add routes ...

    // Use Pingora's built-in compression module (level 6)
    app.add_http_module(ResponseCompressionBuilder::enable(6));

    // HTTP modules work at a lower level than middleware,
    // providing better performance for HTTP processing
}
```

For other HTTP methods, use `add` with a method value:

```rust
use std::sync::Arc;
use pingora_web::{App, Method, Handler, PingoraHttpRequest, PingoraWebHttpResponse, WebError};

struct PutHandler;
#[async_trait::async_trait]
impl Handler for PutHandler {
    async fn handle(&self, _req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        Ok(PingoraWebHttpResponse::no_content())
    }
}

fn main() {
    let mut app = App::default();
    app.add(Method::PUT, "/resource/{id}", Arc::new(PutHandler));
}
```

### Available HTTP Modules

- **ResponseCompressionBuilder**: High-performance gzip compression
  - Supports compression levels 1-9
  - Automatic client detection via Accept-Encoding
  - Streaming compression for large responses
  - Optimized for production use at Cloudflare scale

### Compression Example

```rust
// Enable compression with level 6 (recommended)
app.add_http_module(ResponseCompressionBuilder::enable(6));

// Test with curl:
// curl -H "Accept-Encoding: gzip" -v http://localhost:8080/large-response
// Response will include: Content-Encoding: gzip
```

### Static File Serving (Optional)

```rust
use std::sync::Arc;
use pingora_web::{App};
use pingora_web::utils::ServeDir;

fn setup_app() -> App {
    let mut app = App::default();
    // Serve static files from ./public directory
    app.get("/static/{path}", Arc::new(ServeDir::new("./public")));
    app
}
```

<!-- SSE section removed; feature not provided out-of-the-box -->

## Development

### Prerequisites

- Rust 1.75 or later
- Git

### Building

```bash
git clone https://github.com/pingora-web/pingora_web.git
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



## 🎯 Use Cases

Perfect for:
- **APIs and microservices** requiring high throughput
- **Edge applications** with low latency requirements
- **Proxy servers** and load balancers
- **Real-time applications** with many concurrent connections

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

Source repository: https://github.com/pingora-web/pingora_web
Documentation: https://docs.rs/pingora_web

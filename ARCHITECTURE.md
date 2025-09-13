# 🏗️ pingora_web Architecture

This document explains the design philosophy and architectural decisions behind pingora_web.

## Overview

pingora_web is a web framework built on Cloudflare's Pingora proxy infrastructure, designed to be fast, reliable, and easy to use. It provides a clean abstraction layer over Pingora's powerful but complex HTTP handling capabilities.

## Design Philosophy

### Layered Abstraction

```
┌─────────────────────────────────────────┐
│ Application Layer (Handlers/Middleware) │
├─────────────────────────────────────────┤
│ pingora_web (Routing, Composition)      │
├─────────────────────────────────────────┤
│ Pingora Core (HTTP, Proxy, Performance) │
└─────────────────────────────────────────┘
```

### Core Principles

1. **Progressive Complexity**:
   - Simple: `app.listen()` for beginners
   - Advanced: `app.to_service()` for full control

2. **Composable Building Blocks**:
   - Handlers for business logic
   - Middleware for cross-cutting concerns
   - HTTP Modules for protocol-level features

3. **Performance First**:
   - Zero-copy where possible
   - Arc-based sharing for efficiency
   - Direct integration with Pingora's optimized HTTP stack

4. **Type Safety**:
   - Compile-time route validation
   - Strong typing throughout the request lifecycle
   - Async/await native support

## Handler Design

pingora_web uses a trait-based Handler design that prioritizes simplicity, flexibility, and performance:

```rust
#[async_trait]
pub trait Handler: Send + Sync + 'static {
    async fn handle(&self, req: Request) -> Response;
}
```

### Design Considerations

1. **Simple Interface**: Single method `handle(req) -> Response` keeps the API minimal and focused
2. **Async by Default**: Native async support for I/O operations without blocking threads
3. **Type Safety**: Strong typing prevents many runtime errors at compile time
4. **Zero-cost Abstractions**: Trait objects allow polymorphism without performance penalty
5. **Composability**: Handlers can be easily wrapped in middleware or combined

### Benefits

- **Easy to Test**: Handlers are pure functions that can be tested in isolation
- **Reusable**: Same handler can be used across multiple routes
- **Stateful**: Handlers can hold configuration, database connections, etc.
- **Middleware-Friendly**: Handlers integrate seamlessly with the middleware system

### Example Implementation

```rust
use pingora_web::StatusCode;

struct UserHandler {
    db: Arc<Database>,  // Stateful handler with dependencies
}

#[async_trait]
impl Handler for UserHandler {
    async fn handle(&self, req: Request) -> Response {
        let user_id = req.param("id").unwrap_or("0");
        match self.db.find_user(user_id).await {
            Ok(user) => Response::json(StatusCode::OK, user),
            Err(_) => Response::text(StatusCode::NOT_FOUND, "User not found"),
        }
    }
}
```

## Middleware Design

pingora_web implements an "onion model" middleware system similar to Express.js:

```rust
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    async fn handle(&self, req: Request, next: Arc<dyn Handler>) -> Response;
}
```

### Key Design Decisions

1. **Onion Model**: Middleware wraps around handlers, allowing pre/post processing
2. **Explicit Next**: Middleware must explicitly call `next.handle(req)` for control flow
3. **Composable**: Multiple middleware can be chained together seamlessly
4. **Handler Compatibility**: Middleware and handlers share the same base interface

### Execution Order

```
Request → Middleware A → Middleware B → Handler → Middleware B → Middleware A → Response
```

### Benefits

- **Flexible Control**: Middleware can modify requests, responses, or abort processing
- **Cross-cutting Concerns**: Authentication, logging, compression handled cleanly
- **Performance**: Zero-cost composition with Arc references
- **Testable**: Each middleware can be tested independently

## Router Design

The router uses the `matchit` crate for efficient path matching with parameter extraction:

```rust
pub struct Router {
    by_method: HashMap<String, matchit::Router<Arc<dyn Handler>>>,
}
```

### Features

- **HTTP Method Separation**: Each HTTP method has its own routing table
- **Path Parameters**: Support for `/users/{id}` style routes
- **Efficient Matching**: O(log n) lookup time with radix tree
- **HEAD Fallback**: Automatic HEAD support for GET routes (RFC compliant)
- **Method Not Allowed**: Proper 405 responses with Allow header

## Request/Response Model

### Request

The `Request` type provides:
- HTTP method and path
- Headers and body access
- Route parameters
- Shared data (app-level and request-level)

### Response

The `Response` type supports:
- Status codes and headers
- Both byte and streaming bodies
- Automatic content-length/transfer-encoding headers
- JSON serialization helpers

## Integration with Pingora

pingora_web implements Pingora's `HttpServerApp` trait to integrate seamlessly:

```rust
#[async_trait]
impl HttpServerApp for App {
    async fn process_new_http(
        self: &Arc<Self>,
        mut http: ServerSession,
        shutdown: &ShutdownWatch,
    ) -> Option<ReusedHttpStream>
}
```

### Benefits of Pingora Integration

1. **Production-Ready**: Leverages Cloudflare's battle-tested HTTP stack
2. **HTTP Modules**: Access to Pingora's high-performance modules (compression, etc.)
3. **Connection Pooling**: Automatic HTTP/1.1 and HTTP/2 connection reuse
4. **Graceful Shutdown**: Built-in shutdown handling
5. **Performance**: Zero-copy operations where possible

## Performance Considerations

### Memory Management

- **Arc References**: Shared ownership without copying
- **Zero-Copy**: Direct access to Pingora's byte buffers
- **Streaming**: Support for large responses without memory bloat

### Concurrency

- **Thread-Safe**: All components are Send + Sync
- **Async**: Non-blocking I/O throughout
- **Connection Pooling**: Efficient connection reuse

### Compilation

- **Static Dispatch**: Trait objects only where necessary
- **Inlining**: Small functions get inlined for performance
- **Zero-Cost**: Abstractions compile away

## Future Considerations

### Extensibility

The current design allows for future extensions:

- Custom HTTP modules
- Plugin system for handlers
- Additional middleware types
- Custom routing strategies

### Compatibility

The framework maintains compatibility with:

- Standard HTTP semantics
- Pingora's module system
- Rust's async ecosystem
- Common web framework patterns

## Conclusion

This architecture balances ease of use with performance and flexibility. By leveraging Rust's type system and Pingora's optimized HTTP stack, pingora_web provides a solid foundation for high-performance web applications while remaining approachable for beginners.
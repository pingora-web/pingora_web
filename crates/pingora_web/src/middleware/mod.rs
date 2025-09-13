#![allow(clippy::module_inception)]
pub mod middleware;
pub mod request_id_middleware;
pub mod tracing_middleware;
pub mod limits_middleware;
pub mod panic_recovery_middleware;
pub mod compression_middleware;

pub use middleware::{Middleware, compose};
pub use request_id_middleware::RequestId;
pub use tracing_middleware::TracingMiddleware;
pub use limits_middleware::{LimitsMiddleware, LimitsConfig};
pub use panic_recovery_middleware::PanicRecoveryMiddleware;
pub use compression_middleware::{CompressionMiddleware, CompressionConfig, CompressionAlgorithm};

use crate::logging::{Level, Logger};
use tracing::{debug, error, info, trace, warn};

/// A logger implementation that uses the tracing crate
#[derive(Debug, Clone, Default)]
pub struct TracingLogger;

impl TracingLogger {
    pub fn new() -> Self {
        Self
    }
}

impl Logger for TracingLogger {
    fn log(&self, level: Level, msg: &str, request_id: &str) {
        match level {
            Level::Error => error!(request_id = request_id, "{}", msg),
            Level::Warn => warn!(request_id = request_id, "{}", msg),
            Level::Info => info!(request_id = request_id, "{}", msg),
            Level::Debug => debug!(request_id = request_id, "{}", msg),
            Level::Trace => trace!(request_id = request_id, "{}", msg),
        }
    }
}

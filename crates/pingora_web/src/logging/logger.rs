#[derive(Debug, Clone, Copy)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub trait Logger: Send + Sync {
    fn log(&self, level: Level, message: &str, request_id: &str);
}

pub struct StdoutLogger;

impl Logger for StdoutLogger {
    fn log(&self, level: Level, message: &str, request_id: &str) {
        eprintln!(
            "level={:?} request_id={} msg=\"{}\"",
            level, request_id, message
        );
    }
}

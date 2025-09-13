use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn generate() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Simple, collision-resistant enough for single-process: base36 timestamp + counter
    format!("{:x}-{:x}", ts, c)
}

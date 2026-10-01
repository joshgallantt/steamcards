use std::sync::Arc;

use chrono::{DateTime, Utc};

/// What time it is. The real clock is the system's; a test's can move with
/// tokio's paused time, so hours of pricing pass in moments.
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// The system's clock.
pub fn system_clock() -> Clock {
    Arc::new(Utc::now)
}

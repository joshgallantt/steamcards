use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};

use crate::Clock;

/// A clock that reads `start` now, and moves with tokio's time from here:
/// on paused time, hours pass in moments.
pub fn clock_from(start: DateTime<Utc>) -> Clock {
    let started = tokio::time::Instant::now();
    Arc::new(move || start + TimeDelta::from_std(started.elapsed()).unwrap_or_default())
}

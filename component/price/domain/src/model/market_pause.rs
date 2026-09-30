use std::time::Duration;

use chrono::{DateTime, Utc};

/// Steam's pause on price lookups. When Steam turns one down, no lookup
/// goes until the pause ends; then one goes to see, and if that's turned
/// down too, the pause is longer. It outlasts a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketPause {
    pub until: DateTime<Utc>,
    /// How long this pause is: 10 minutes at first, doubling each time
    /// Steam turns a lookup down again, to an hour at most.
    pub step: Duration,
}

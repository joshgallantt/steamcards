use std::time::Duration;

use chrono::DateTime;
use price::MarketPause;
use serde::{Deserialize, Serialize};

/// Steam's pause on market requests, as kept, to the second.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MarketPauseDto {
    /// When it ends, in seconds since the epoch.
    until: i64,
    /// How long it is, in seconds.
    step: u64,
}

impl MarketPauseDto {
    pub(crate) fn new(pause: MarketPause) -> Self {
        Self {
            until: pause.until.timestamp(),
            step: pause.step.as_secs(),
        }
    }

    pub(crate) fn into_domain(self) -> MarketPause {
        MarketPause {
            until: DateTime::from_timestamp(self.until, 0).unwrap_or_default(),
            step: Duration::from_secs(self.step),
        }
    }
}

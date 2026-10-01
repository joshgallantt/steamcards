use chrono::{DateTime, Utc};

use crate::{
    PriceQuote,
    model::rules::{RETRY_FAILED, later},
};

/// What's known of a card's price (research §1.5). Only a known price ever
/// counts: one that isn't known is never taken as nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Price {
    /// Not looked up yet.
    Pending,
    Known(PriceQuote),
    /// It can be sold, but nobody is selling it.
    NoMarket,
    /// It can't be sold on the market at all.
    NotMarketable,
    /// Steam's answer couldn't be used. It's looked up again then.
    Failed {
        retry_at: DateTime<Utc>,
    },
}

impl Price {
    /// A lookup that failed at `at`: it's tried again a day later.
    pub fn failed(at: DateTime<Utc>) -> Price {
        Price::Failed {
            retry_at: later(at, RETRY_FAILED),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    fn noon() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    #[test]
    fn a_failed_lookup_is_tried_again_a_day_later() {
        assert_eq!(
            Price::failed(noon()),
            Price::Failed {
                retry_at: noon() + TimeDelta::hours(24)
            }
        );
    }
}

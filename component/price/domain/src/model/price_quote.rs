use std::time::Duration;

use chrono::{DateTime, Utc};
use money::Money;

use crate::{
    QuoteSource,
    rules::{FRESH_FOR, between},
};

/// What the market said a card sells for, and when. Both prices are what a
/// buyer pays, fees included.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceQuote {
    /// The lowest listing.
    pub ask: Option<Money>,
    /// The best offer: what someone is waiting to pay for it.
    pub bid: Option<Money>,
    /// How many are listed.
    pub ask_depth: Option<u32>,
    /// How many are wanted.
    pub bid_depth: Option<u32>,
    pub source: QuoteSource,
    pub fetched_at: DateTime<Utc>,
}

impl PriceQuote {
    /// How old it is at `now`.
    pub fn age(&self, now: DateTime<Utc>) -> Duration {
        between(self.fetched_at, now)
    }

    /// Older than 6 hours: it's shown dim with its age, and still counts.
    pub fn is_stale(&self, now: DateTime<Utc>) -> bool {
        self.age(now) > FRESH_FOR
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use money::Currency;

    use super::*;

    const HOUR: Duration = Duration::from_secs(60 * 60);

    fn noon() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    #[test]
    fn a_quote_over_six_hours_old_is_stale() {
        let quote = PriceQuote {
            ask: Some(Money::new(5, Currency::GBP)),
            bid: None,
            ask_depth: Some(10),
            bid_depth: None,
            source: QuoteSource::Search,
            fetched_at: noon(),
        };
        let at = |hours: i64| noon() + TimeDelta::hours(hours);
        assert!(!quote.is_stale(at(6)));
        assert!(quote.is_stale(at(8)));
        assert_eq!(quote.age(at(8)), 8 * HOUR);
        assert_eq!(quote.age(at(-1)), Duration::ZERO, "a clock put back");
    }
}

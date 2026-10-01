use chrono::{DateTime, Utc};
use money::{Currency, Money};

use crate::{Price, PriceQuote, QuoteSource};

/// A card's lowest listing on the market: `ask` pence, with `listings`
/// listed, as looked up at `at`.
pub fn listing(ask: i64, listings: u32, at: DateTime<Utc>) -> Price {
    Price::Known(PriceQuote {
        ask: Some(Money::new(ask, Currency::GBP)),
        bid: None,
        ask_depth: Some(listings),
        bid_depth: None,
        source: QuoteSource::Search,
        fetched_at: at,
    })
}

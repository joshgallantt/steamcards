use chrono::{DateTime, Utc};
use money::{Currency, Money};

use crate::{Price, PriceQuote, QuoteSource};

/// A card's order book looked up at `at`: its lowest listing and its best
/// offer, in pence.
pub fn order_book(ask: i64, bid: i64, at: DateTime<Utc>) -> Price {
    Price::Known(PriceQuote {
        ask: Some(Money::new(ask, Currency::GBP)),
        bid: Some(Money::new(bid, Currency::GBP)),
        ask_depth: Some(20),
        bid_depth: Some(100),
        source: QuoteSource::OrderBook,
        fetched_at: at,
    })
}

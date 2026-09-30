use chrono::DateTime;
use money::{Currency, Money};
use price::{Price, PriceQuote, QuoteSource};
use serde::{Deserialize, Serialize};

/// A price, as kept. `state` is "known", "no market", "not marketable",
/// "failed" or "pending"; the rest is what a known price, or a failed
/// lookup, has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PriceDto {
    state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ask: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ask_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bid_depth: Option<u32>,
    /// Steam's `ECurrency` id for the amounts.
    #[serde(default)]
    currency: u32,
    /// "search" or "order book".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source: String,
    /// In seconds since the epoch.
    #[serde(default)]
    fetched_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retry_at: Option<i64>,
}

impl PriceDto {
    pub(crate) fn new(price: &Price) -> Self {
        let state = |state: &str| Self {
            state: state.to_owned(),
            ..Self::default()
        };
        match price {
            Price::Pending => state("pending"),
            Price::NoMarket => state("no market"),
            Price::NotMarketable => state("not marketable"),
            Price::Failed { retry_at } => Self {
                retry_at: Some(retry_at.timestamp()),
                ..state("failed")
            },
            Price::Known(quote) => Self {
                ask: quote.ask.map(|m| m.minor),
                bid: quote.bid.map(|m| m.minor),
                ask_depth: quote.ask_depth,
                bid_depth: quote.bid_depth,
                currency: quote.ask.or(quote.bid).map_or(0, |m| m.currency.id()),
                source: match quote.source {
                    QuoteSource::Search => "search",
                    QuoteSource::OrderBook => "order book",
                }
                .to_owned(),
                fetched_at: quote.fetched_at.timestamp(),
                ..state("known")
            },
        }
    }

    /// `None` for a state it doesn't know, or a time that doesn't read.
    pub(crate) fn to_domain(&self) -> Option<Price> {
        let time = |seconds| DateTime::from_timestamp(seconds, 0);
        Some(match self.state.as_str() {
            "pending" => Price::Pending,
            "no market" => Price::NoMarket,
            "not marketable" => Price::NotMarketable,
            "failed" => Price::Failed {
                retry_at: time(self.retry_at?)?,
            },
            "known" => {
                let currency = Currency::from_id(self.currency);
                Price::Known(PriceQuote {
                    ask: self.ask.map(|minor| Money::new(minor, currency)),
                    bid: self.bid.map(|minor| Money::new(minor, currency)),
                    ask_depth: self.ask_depth,
                    bid_depth: self.bid_depth,
                    source: match self.source.as_str() {
                        "order book" => QuoteSource::OrderBook,
                        _ => QuoteSource::Search,
                    },
                    fetched_at: time(self.fetched_at)?,
                })
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }

    fn known(ask: Option<i64>, bid: Option<i64>, source: QuoteSource) -> Price {
        Price::Known(PriceQuote {
            ask: ask.map(|minor| Money::new(minor, Currency::GBP)),
            bid: bid.map(|minor| Money::new(minor, Currency::GBP)),
            ask_depth: Some(2005),
            bid_depth: bid.map(|_| 34378),
            source,
            fetched_at: at(1_790_700_000),
        })
    }

    #[test]
    fn every_price_is_kept_as_it_was() {
        let prices = [
            Price::Pending,
            Price::NoMarket,
            Price::NotMarketable,
            Price::failed(at(1_790_700_000)),
            known(Some(11), Some(8), QuoteSource::OrderBook),
            known(Some(6), None, QuoteSource::Search),
        ];
        for price in prices {
            assert_eq!(
                PriceDto::new(&price).to_domain(),
                Some(price.clone()),
                "{price:?}"
            );
        }
        assert_eq!(
            PriceDto::default().to_domain(),
            None,
            "a state it doesn't know"
        );
    }
}

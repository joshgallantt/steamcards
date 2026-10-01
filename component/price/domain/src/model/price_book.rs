use std::collections::BTreeMap;

use steam_library::AppId;

use crate::{Basis, HeldCard, Offers, Price, SetPrices};

/// Everything priced so far: each game's set, and each card's best offers
/// that have been looked up.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PriceBook {
    /// Sets, by app ID.
    pub sets: BTreeMap<AppId, SetPrices>,
    /// Order books, by market hash name.
    pub offers: BTreeMap<String, Offers>,
}

impl PriceBook {
    /// A held card's price on `basis`. List and net come from its game's
    /// set, instant from its order book. A held card is worth the latest
    /// price for it, not the one it had when it dropped.
    pub fn price(&self, card: &HeldCard, basis: Basis) -> Price {
        if !card.marketable {
            return Price::NotMarketable;
        }
        let set = self.sets.get(&card.app_id);
        match basis {
            Basis::List | Basis::Net => {
                let Some(set) = set else {
                    return Price::Pending;
                };
                let listed = card
                    .market_hash_name
                    .as_deref()
                    .and_then(|hash| set.by_hash(hash))
                    .or_else(|| set.card(&card.name, card.foil));
                listed.map_or_else(|| set.unlisted(), |c| c.price.clone())
            }
            Basis::Instant => {
                // A card known only by its name has its hash name from its set.
                let hash = card.market_hash_name.as_deref().or_else(|| {
                    set?.card(&card.name, card.foil)
                        .map(|c| c.market_hash_name.as_str())
                });
                hash.and_then(|h| self.offers.get(h))
                    .map_or(Price::Pending, |offers| offers.price.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use money::{Currency, Money};

    use super::*;
    use crate::{PriceQuote, PricedCard, QuoteSource};

    fn noon() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn listed(name: &str, hash: &str, ask: i64) -> PricedCard {
        PricedCard {
            name: name.into(),
            market_hash_name: hash.into(),
            price: Price::Known(PriceQuote {
                ask: Some(Money::new(ask, Currency::GBP)),
                bid: None,
                ask_depth: Some(10),
                bid_depth: None,
                source: QuoteSource::Search,
                fetched_at: noon(),
            }),
        }
    }

    #[test]
    fn a_held_cards_price_depends_on_the_basis() {
        let mut book = PriceBook::default();
        book.sets.insert(
            AppId(960_910),
            SetPrices {
                app_id: AppId(960_910),
                normal: vec![listed("Madison", "960910-Madison", 5)],
                foil: Vec::new(),
                fetched_at: noon(),
                retry_at: None,
            },
        );
        let madison = HeldCard {
            market_hash_name: Some("960910-Madison".into()),
            ..HeldCard::named(AppId(960_910), "Madison", false)
        };
        let by_name = HeldCard::named(AppId(960_910), "Madison", false);
        assert!(matches!(book.price(&madison, Basis::List), Price::Known(_)));
        assert_eq!(
            book.price(&madison, Basis::Net),
            book.price(&by_name, Basis::Net)
        );
        assert_eq!(
            book.price(&madison, Basis::Instant),
            Price::Pending,
            "no order book yet"
        );
        book.offers.insert(
            "960910-Madison".into(),
            Offers {
                price: Price::NoMarket,
                looked_up_at: noon(),
            },
        );
        assert_eq!(
            book.price(&by_name, Basis::Instant),
            Price::NoMarket,
            "its hash from its set"
        );

        let scott = HeldCard::named(AppId(960_910), "Scott", false);
        assert_eq!(book.price(&scott, Basis::List), Price::NoMarket);
        let hades = HeldCard::named(AppId(1_145_360), "Zagreus", false);
        assert_eq!(book.price(&hades, Basis::List), Price::Pending);
        let unsellable = HeldCard {
            marketable: false,
            ..madison
        };
        assert_eq!(book.price(&unsellable, Basis::List), Price::NotMarketable);
    }
}

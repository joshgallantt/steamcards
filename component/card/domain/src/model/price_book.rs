use std::collections::BTreeMap;

use game::AppId;

use crate::{HeldCard, Price, SetPrices};

/// Everything priced so far: each game's set, by app ID.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PriceBook {
    pub sets: BTreeMap<AppId, SetPrices>,
}

impl PriceBook {
    /// A held card's price, from its game's set: by its market hash name
    /// when its copy was described, else by its name. A held card is worth
    /// the latest price for it, not the one it had when it dropped.
    pub fn price(&self, card: &HeldCard) -> Price {
        if !card.marketable {
            return Price::NotMarketable;
        }
        let Some(set) = self.sets.get(&card.app_id) else {
            return Price::Pending;
        };
        let listed = card
            .market_hash_name
            .as_deref()
            .and_then(|hash| set.by_hash(hash))
            .or_else(|| set.card(&card.name, card.kind));
        listed.map_or_else(|| set.unlisted(), |c| c.price.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::CardKind;
    use chrono::{DateTime, Utc};
    use money::{Currency, Money};

    use super::*;
    use crate::{PriceQuote, PricedCard};

    fn noon() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn known(ask: i64) -> Price {
        Price::Known(PriceQuote {
            ask: Some(Money::new(ask, Currency::GBP)),
            ask_depth: Some(10),
            fetched_at: noon(),
        })
    }

    fn book_with_madison() -> PriceBook {
        let mut book = PriceBook::default();
        book.sets.insert(
            AppId(960_910),
            SetPrices {
                app_id: AppId(960_910),
                normal: vec![PricedCard {
                    name: "Madison".into(),
                    market_hash_name: "960910-Madison".into(),
                    price: known(5),
                }],
                foil: Vec::new(),
                fetched_at: noon(),
                retry_at: None,
            },
        );
        book
    }

    #[test]
    fn a_held_card_is_priced_from_its_set() {
        let book = book_with_madison();
        let described = HeldCard {
            market_hash_name: Some("960910-Madison".into()),
            ..HeldCard::named(AppId(960_910), "Madison", CardKind::Normal)
        };
        let by_name = HeldCard::named(AppId(960_910), "Madison", CardKind::Normal);
        assert_eq!(book.price(&described), known(5));
        assert_eq!(book.price(&by_name), known(5), "by its name in the set");
    }

    #[test]
    fn what_isnt_priced_yet_or_cant_be_sold_says_so() {
        let book = book_with_madison();
        let scott = HeldCard::named(AppId(960_910), "Scott", CardKind::Normal);
        let hades = HeldCard::named(AppId(1_145_360), "Zagreus", CardKind::Normal);
        let unsellable = HeldCard {
            marketable: false,
            ..HeldCard::named(AppId(960_910), "Madison", CardKind::Normal)
        };
        assert_eq!(book.price(&scott), Price::NoMarket, "not listed");
        assert_eq!(
            book.price(&hades),
            Price::Pending,
            "its set isn't priced yet"
        );
        assert_eq!(book.price(&unsellable), Price::NotMarketable);
    }
}

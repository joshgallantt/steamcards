use crate::CardKind;
use chrono::{DateTime, Utc};
use game::AppId;

use crate::{
    Price, PricedCard,
    model::rules::{FRESH_FOR, RETRY_FAILED, later},
};

/// A game's set of cards, priced: its normal cards and its foils, from one
/// lookup of each kind at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct SetPrices {
    pub app_id: AppId,
    pub normal: Vec<PricedCard>,
    pub foil: Vec<PricedCard>,
    /// When its prices were looked up; for a set whose only lookup failed,
    /// when that was.
    pub fetched_at: DateTime<Utc>,
    /// Its last lookup failed, and it's tried again then. The prices it
    /// has, if any, are from the lookup before.
    pub retry_at: Option<DateTime<Utc>>,
}

impl SetPrices {
    /// A set whose first lookup failed at `at`: nothing priced, and tried
    /// again a day later.
    pub fn failed(app_id: AppId, at: DateTime<Utc>) -> Self {
        Self {
            app_id,
            normal: Vec::new(),
            foil: Vec::new(),
            fetched_at: at,
            retry_at: Some(later(at, RETRY_FAILED)),
        }
    }

    /// The set's cards of one kind, priced.
    pub fn cards(&self, kind: CardKind) -> &[PricedCard] {
        match kind {
            CardKind::Normal => &self.normal,
            CardKind::Foil => &self.foil,
        }
    }

    /// A card of the set by its name as the game's set lists it, and its
    /// kind.
    pub fn card(&self, name: &str, kind: CardKind) -> Option<&PricedCard> {
        self.cards(kind).iter().find(|c| c.name == name)
    }

    /// A card of the set by its market hash name.
    pub fn by_hash(&self, market_hash_name: &str) -> Option<&PricedCard> {
        self.normal
            .iter()
            .chain(&self.foil)
            .find(|c| c.market_hash_name == market_hash_name)
    }

    /// What a card of the set sells for, by its name and kind.
    pub fn price(&self, name: &str, kind: CardKind) -> Price {
        self.card(name, kind)
            .map_or_else(|| self.unlisted(), |c| c.price.clone())
    }

    /// The price of a card of the set the market didn't list: failed, if
    /// the lookup did. Otherwise nobody is selling it. Whether the market
    /// lists a card nobody is selling at all is an open question (research
    /// §6, question 3); either way, it has no market.
    pub(crate) fn unlisted(&self) -> Price {
        match self.retry_at {
            Some(retry_at) => Price::Failed { retry_at },
            None => Price::NoMarket,
        }
    }

    /// When it's next due a lookup: when its prices turn 6 hours old, or
    /// after a failed lookup, a day after it.
    pub fn due_at(&self) -> DateTime<Utc> {
        self.retry_at
            .unwrap_or_else(|| later(self.fetched_at, FRESH_FOR))
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use money::{Currency, Money};

    use super::*;
    use crate::PriceQuote;

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
                ask_depth: Some(10),
                fetched_at: noon(),
            }),
        }
    }

    #[test]
    fn a_sets_cards_are_found_by_name_and_by_hash_name() {
        let portal = SetPrices {
            app_id: AppId(620),
            normal: vec![
                listed("Intro", "620-Intro (Trading Card)", 8),
                listed("Chell", "620-Chell", 6),
            ],
            foil: vec![listed("Intro", "620-Intro (Foil Trading Card)", 63)],
            fetched_at: noon(),
            retry_at: None,
        };
        assert_eq!(
            portal
                .card("Intro", CardKind::Foil)
                .unwrap()
                .market_hash_name,
            "620-Intro (Foil Trading Card)"
        );
        assert_eq!(portal.by_hash("620-Chell").unwrap().name, "Chell");
        assert_eq!(portal.card("Chell", CardKind::Foil), None);
        assert_eq!(
            portal.price("Atlas", CardKind::Normal),
            Price::NoMarket,
            "not listed"
        );
        assert_eq!(portal.due_at(), noon() + TimeDelta::hours(6));

        let failed = SetPrices {
            retry_at: Some(noon() + TimeDelta::hours(30)),
            ..portal
        };
        assert!(
            matches!(failed.price("Chell", CardKind::Normal), Price::Known(_)),
            "the prices from before"
        );
        assert_eq!(
            failed.price("Atlas", CardKind::Normal),
            Price::Failed {
                retry_at: noon() + TimeDelta::hours(30)
            }
        );
        assert_eq!(failed.due_at(), noon() + TimeDelta::hours(30));
        assert_eq!(
            SetPrices::failed(AppId(620), noon()).due_at(),
            noon() + TimeDelta::hours(24)
        );
    }
}

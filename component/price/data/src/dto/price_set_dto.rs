use chrono::DateTime;
use game::AppId;
use price::SetPrices;
use serde::{Deserialize, Serialize};

use crate::dto::PricedCardDto;

/// A game's set of cards, priced, as kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PriceSetDto {
    app_id: u32,
    /// When it was looked up, in seconds since the epoch.
    fetched_at: i64,
    /// When its last lookup failed, it's tried again then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retry_at: Option<i64>,
    #[serde(default)]
    normal: Vec<PricedCardDto>,
    #[serde(default)]
    foil: Vec<PricedCardDto>,
}

impl PriceSetDto {
    pub(crate) fn new(set: &SetPrices) -> Self {
        Self {
            app_id: set.app_id.0,
            fetched_at: set.fetched_at.timestamp(),
            retry_at: set.retry_at.map(|at| at.timestamp()),
            normal: set.normal.iter().map(PricedCardDto::new).collect(),
            foil: set.foil.iter().map(PricedCardDto::new).collect(),
        }
    }

    /// The set as it was kept; `None` when any of it doesn't read, so it's
    /// looked up afresh.
    pub(crate) fn into_domain(self) -> Option<SetPrices> {
        let cards = |cards: Vec<PricedCardDto>| {
            cards
                .into_iter()
                .map(PricedCardDto::into_domain)
                .collect::<Option<Vec<_>>>()
        };
        Some(SetPrices {
            app_id: AppId(self.app_id),
            normal: cards(self.normal)?,
            foil: cards(self.foil)?,
            fetched_at: DateTime::from_timestamp(self.fetched_at, 0)?,
            retry_at: match self.retry_at {
                Some(at) => Some(DateTime::from_timestamp(at, 0)?),
                None => None,
            },
        })
    }
}

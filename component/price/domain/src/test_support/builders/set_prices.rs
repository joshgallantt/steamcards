use card::CardKind;
use chrono::{DateTime, Utc};
use steam_library::AppId;

use crate::{
    SetPrices,
    test_support::{listing, priced_card},
};

/// A game's set, priced at `at`: its normal cards and its foils, each at a
/// list price in pence.
pub fn set_prices(
    app_id: u32,
    normal: &[(&str, i64)],
    foil: &[(&str, i64)],
    at: DateTime<Utc>,
) -> SetPrices {
    let cards = |cards: &[(&str, i64)], kind: CardKind| {
        cards
            .iter()
            .map(|&(name, ask)| priced_card(app_id, name, kind, listing(ask, 50, at)))
            .collect()
    };
    SetPrices {
        app_id: AppId(app_id),
        normal: cards(normal, CardKind::Normal),
        foil: cards(foil, CardKind::Foil),
        fetched_at: at,
        retry_at: None,
    }
}

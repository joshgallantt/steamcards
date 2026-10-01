use crate::{Price, PricedCard};

/// A card of `app_id`'s set as the market lists it: a normal card, or a
/// foil, with the hash name Steam gives it.
pub fn priced_card(app_id: u32, name: &str, foil: bool, price: Price) -> PricedCard {
    let border = if foil { " (Foil)" } else { "" };
    PricedCard {
        name: name.to_owned(),
        market_hash_name: format!("{app_id}-{name}{border}"),
        price,
    }
}

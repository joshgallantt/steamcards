use crate::Card;

/// A card of a set, with `owned` copies held.
pub fn card(name: &str, owned: u32) -> Card {
    Card {
        name: name.to_owned(),
        owned,
    }
}

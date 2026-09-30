use crate::Price;

/// A card of a game's set as the market lists it, and its price.
#[derive(Debug, Clone, PartialEq)]
pub struct PricedCard {
    /// Its name as the game's set lists it: "Intro", where the market
    /// lists "Intro (Trading Card)".
    pub name: String,
    /// Its name on the market, exactly as Steam gives it.
    pub market_hash_name: String,
    pub price: Price,
}

/// One copy of a trading card the account holds: an item in its Steam
/// inventory. Its identity is its asset ID. Each copy that drops is its own
/// asset, so a card that drops twice is two of these.
///
/// It's what says which card dropped, and what selling one takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAsset {
    /// The item's ID in the account's inventory.
    pub asset_id: u64,
    /// The game whose set the card is from.
    pub app_id: u32,
    /// The card's name as the game's set lists it: "Anarchist", never
    /// "Anarchist (Trading Card)" as the market has it.
    pub name: String,
    /// The card's name on the Steam market, exactly as Steam gives it:
    /// "730-Anarchist (Trading Card)".
    pub market_hash_name: String,
    /// A foil: a rarer copy, with a shiny border. Foils make a badge of
    /// their own, so the set's counts leave them out.
    pub foil: bool,
    /// Whether it can be sold on the Steam market.
    pub marketable: bool,
    /// Whether it can be traded.
    pub tradable: bool,
}

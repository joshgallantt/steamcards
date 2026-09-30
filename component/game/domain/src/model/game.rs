use crate::CardDrops;

/// A game on the account that has trading cards. Its identity is its Steam
/// app ID.
///
/// Its drops are what farming works through. Its set, which a badge needs,
/// is a measure of its own: the same card can drop twice, so 3 of 4 drops
/// can be 2 of 5 cards and a spare.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub app_id: u32,
    pub name: String,
    /// Hours on record, as Steam counts them.
    pub hours: f64,
    /// Card drops so far, and still to come from playing it.
    pub drops: CardDrops,
    /// The level of the badge crafted from its cards: 0 until one is.
    pub badge_level: u8,
}

impl Game {
    pub fn has_drops_left(&self) -> bool {
        self.drops.remaining > 0
    }
}

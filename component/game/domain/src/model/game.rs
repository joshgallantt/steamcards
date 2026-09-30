use crate::{AppId, CardDrops, HOURS_BEFORE_DROPS};

/// A game on the account that has trading cards. Its identity is its Steam
/// app ID.
///
/// Its drops are what farming works through. Its set, which a badge needs,
/// is a measure of its own: the same card can drop twice, so 3 of 4 drops
/// can be 2 of 5 cards and a spare.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub app_id: AppId,
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

    /// Hours it still needs on record before its cards can drop: none once
    /// it has 3.
    pub fn hours_to_go(&self) -> f64 {
        (HOURS_BEFORE_DROPS - self.hours).max(0.0)
    }

    /// Whether it has the hours its cards need to start dropping.
    pub fn can_drop(&self) -> bool {
        self.hours_to_go() <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn played(hours: f64) -> Game {
        Game {
            app_id: AppId(620),
            name: "Portal 2".into(),
            hours,
            drops: CardDrops {
                received: 0,
                remaining: 4,
            },
            badge_level: 0,
        }
    }

    #[test]
    fn a_game_needs_three_hours_before_its_cards_drop() {
        assert_eq!(played(0.0).hours_to_go(), 3.0);
        assert!((played(2.2).hours_to_go() - 0.8).abs() < 1e-9, "needs 0.8h");
        assert_eq!(played(3.0).hours_to_go(), 0.0);
        assert_eq!(played(350.0).hours_to_go(), 0.0, "never below none");
        assert!(!played(2.99).can_drop());
        assert!(played(3.0).can_drop());
    }
}

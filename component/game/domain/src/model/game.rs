use chrono::{DateTime, Utc};

use crate::{AppId, CardDrops, REFUND_UNDER_HOURS, REFUND_WITHIN};

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
    /// Marked private in the account's library: Steam drops no cards for
    /// it.
    pub private: bool,
    /// When the account bought it, when that was lately enough that Steam
    /// may still refund it: within [`REFUND_WITHIN`]. A product key, a gift
    /// to itself or a free game isn't bought.
    pub bought_at: Option<DateTime<Utc>>,
}

impl Game {
    pub fn has_drops_left(&self) -> bool {
        self.drops.remaining > 0
    }

    /// Hours it still needs on record before its cards can drop, on an
    /// account that holds cards back for `before_drops` hours: none once it
    /// has them.
    pub fn hours_to_go(&self, before_drops: u8) -> f64 {
        (f64::from(before_drops) - self.hours).max(0.0)
    }

    /// Whether it has the hours its cards need to start dropping, on an
    /// account that holds cards back for `before_drops` hours.
    pub fn can_drop(&self, before_drops: u8) -> bool {
        self.hours_to_go(before_drops) <= 0.0
    }

    /// When Steam stops refunding it, if it was bought lately.
    pub fn refund_ends(&self) -> Option<DateTime<Utc>> {
        self.bought_at.map(|at| at + REFUND_WITHIN)
    }

    /// Whether Steam would still refund it at `now`: bought within the last
    /// 14 days, and played for under 2 hours. Playing it longer would cost
    /// the refund.
    pub fn is_refundable(&self, now: DateTime<Utc>) -> bool {
        self.hours < REFUND_UNDER_HOURS && self.refund_ends().is_some_and(|ends| now < ends)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::TimeDelta;

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
            private: false,
            bought_at: None,
        }
    }

    fn day(n: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_790_553_600, 0).unwrap() + TimeDelta::days(n)
    }

    #[test]
    fn a_game_needs_the_hours_the_account_holds_cards_back_for() {
        assert_eq!(played(0.0).hours_to_go(3), 3.0);
        assert!(
            (played(2.2).hours_to_go(3) - 0.8).abs() < 1e-9,
            "needs 0.8h"
        );
        assert_eq!(played(3.0).hours_to_go(3), 0.0);
        assert_eq!(played(350.0).hours_to_go(3), 0.0, "never below none");
        assert!(!played(2.99).can_drop(3));
        assert!(played(3.0).can_drop(3));
        assert!(played(1.0).can_drop(1));
        assert!(!played(1.0).can_drop(2));
    }

    #[test]
    fn on_an_account_steam_doesnt_hold_back_every_game_can_drop() {
        assert_eq!(played(0.0).hours_to_go(0), 0.0);
        assert!(played(0.0).can_drop(0));
    }

    #[test]
    fn a_game_bought_lately_and_barely_played_can_still_be_refunded() {
        let bought = Game {
            bought_at: Some(day(0)),
            ..played(1.5)
        };
        assert_eq!(bought.refund_ends(), Some(day(14)));
        assert!(bought.is_refundable(day(0)));
        assert!(bought.is_refundable(day(13)));
        assert!(!bought.is_refundable(day(14)), "14 days on, it's too late");
        let played_out = Game {
            hours: 2.0,
            ..bought.clone()
        };
        assert!(!played_out.is_refundable(day(1)), "2 hours played");
        assert!(!played(0.0).is_refundable(day(0)), "not bought lately");
        assert_eq!(played(0.0).refund_ends(), None);
    }
}

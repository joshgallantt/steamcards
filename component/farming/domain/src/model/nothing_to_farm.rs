use chrono::{DateTime, Utc};

/// Why there's nothing to farm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NothingToFarm {
    /// The account has no games with trading cards.
    NoGames,
    /// Every card has dropped.
    AllDropped,
    /// "Only priority" is on, and the priority games are done.
    PrioritiesDone,
    /// Every game with cards left is skipped.
    AllSkipped,
    /// Every game with cards left is left out, some of them for what Steam
    /// says of them: they're `private`, or Steam would still refund them,
    /// the first until `refundable_until`. The rest are skipped.
    HeldBack {
        private: bool,
        refundable_until: Option<DateTime<Utc>>,
    },
    /// Steam isn't dropping cards for the games left: sale events' badges,
    /// and games set aside too often.
    NotDropping,
}

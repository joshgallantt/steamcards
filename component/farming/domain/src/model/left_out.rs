use chrono::{DateTime, Utc};

/// Why a game with cards still to drop isn't farmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftOut {
    /// The user skipped it.
    Skipped,
    /// "Only priority" is on, and it isn't one of the priority games.
    NotPriority,
    /// A sale event's badge: its cards come from taking part in the sale,
    /// not from playing.
    SaleEvent,
    /// Marked private in the account's library, and private games are left
    /// out: Steam drops no cards for them.
    Private,
    /// Steam would still refund it, and such games are left out, so farming
    /// doesn't cost the refund: until Steam stops refunding it, `until`, or
    /// it has been played 2 hours.
    Refundable { until: DateTime<Utc> },
}

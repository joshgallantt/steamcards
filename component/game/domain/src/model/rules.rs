//! The numbers about games that farming and its forecast both run on, each
//! with where it comes from (docs/research/steam-card-farming.md).

use chrono::TimeDelta;

/// Hours a game needs on record before its cards drop, on most accounts:
/// what the user's setting starts at. ASF's and Steam Game Idler's default,
/// and xPaw's 180 minutes. Valve documents no such rule; it's what the
/// farmers observe, and an account Steam doesn't hold back needs none.
pub const HOURS_BEFORE_DROPS: u8 = 3;

/// The most games Steam counts as played at once.
pub const MOST_PLAYED_AT_ONCE: usize = 32;

/// Steam refunds a game bought in the last 14 days, played for under 2
/// hours (Steam's refund policy, store.steampowered.com/steam_refunds; ASF's
/// `DaysForRefund` and `HoursForRefund`).
pub const REFUND_WITHIN: TimeDelta = TimeDelta::days(14);
pub const REFUND_UNDER_HOURS: f64 = 2.0;

//! The numbers the market runs on, each with where it comes from: see
//! docs/research/market-and-session.md and docs/design/ui.md §5.3. How far
//! apart requests go, and how long Steam's pause lasts, are Steam's
//! business: steam-api keeps them.

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

/// A set's prices are fresh for 6 hours; then they're looked up again, and
/// until they are, they show dim with their age and still count (research
/// §1.3's cache).
pub(crate) const FRESH_FOR: Duration = Duration::from_secs(6 * 60 * 60);

/// A lookup whose answer couldn't be used is tried again a day later
/// (research §1.3 and §1.5).
pub(crate) const RETRY_FAILED: Duration = Duration::from_secs(24 * 60 * 60);

/// While the market can't be asked, or doesn't answer (no sign-in, no
/// network, a server error twice), the next lookup waits a minute, then
/// twice as long each time it still can't, up to half an hour: soon, but
/// never quickly again and again. Nothing is taken as failed meanwhile
/// (research §1.3: after a server error, a price is stale, not failed).
pub(crate) const UNANSWERED_FIRST: Duration = Duration::from_secs(60);
pub(crate) const UNANSWERED_LONGEST: Duration = Duration::from_secs(30 * 60);

/// When a card drops, its game's set is looked up again if its prices are
/// over an hour old (ui.md §5.3).
pub(crate) const ASKED_AGAIN_AFTER: Duration = Duration::from_secs(60 * 60);

/// How often the watcher looks at what's wanted, and at the clock, while it
/// waits: so a change shows within moments, as the farmer's tick does, and
/// a computer that slept doesn't leave prices waiting.
pub(crate) const TICK: Duration = Duration::from_secs(30);

/// `by` after `at`.
pub(crate) fn later(at: DateTime<Utc>, by: Duration) -> DateTime<Utc> {
    TimeDelta::from_std(by)
        .ok()
        .and_then(|by| at.checked_add_signed(by))
        .unwrap_or(DateTime::<Utc>::MAX_UTC)
}

/// How long from `from` to `to`: nothing, if `to` comes first.
pub(crate) fn between(from: DateTime<Utc>, to: DateTime<Utc>) -> Duration {
    (to - from).to_std().unwrap_or(Duration::ZERO)
}

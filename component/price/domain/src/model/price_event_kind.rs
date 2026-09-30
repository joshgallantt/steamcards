use chrono::{DateTime, Utc};
use game::AppId;

use crate::MarketPause;

/// What a market event is about, so the UI can show the ones that matter,
/// and write its own line from what each carries: a game's name for its app
/// ID, a time on the clock for a time. The event's message says the same,
/// in the market's own words, with durations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PriceEventKind {
    /// Every game wanted priced has been looked up, for now: how many, and
    /// when the next round is due.
    AllPriced {
        games: usize,
        next_round: Option<DateTime<Utc>>,
    },
    /// Steam turned a lookup down: lookups wait until the pause ends.
    Paused(MarketPause),
    /// Steam's pause is over: the first lookup since wasn't turned down.
    Resumed,
    /// A game's prices couldn't be looked up, by app ID: the market's
    /// answer couldn't be used. They're tried again a day later.
    Failed(AppId),
    /// The market couldn't be asked, or didn't answer: it's asked again
    /// then, and nothing is taken as failed meanwhile.
    Unanswered { retry_at: DateTime<Utc> },
}

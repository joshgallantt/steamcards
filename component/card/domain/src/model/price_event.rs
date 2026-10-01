use std::time::Duration;

use chrono::{DateTime, Utc};
use game::AppId;

use crate::MarketPause;

/// What the background pricing reports, as it happens, for the log. It says
/// what happened, with what the screens need to word it: a game by its app
/// ID, a time as a time.
#[derive(Debug, Clone, PartialEq)]
pub enum PriceEvent {
    /// Every game wanted priced has been looked up, for now, as of `at`:
    /// how many, and when the next round is due.
    AllPriced {
        games: usize,
        next_round: Option<DateTime<Utc>>,
        at: DateTime<Utc>,
    },
    /// Steam turned a lookup down: lookups wait until the pause ends.
    /// `again` when it turned down the lookup sent to see whether the last
    /// pause was over.
    Paused { pause: MarketPause, again: bool },
    /// Steam's pause is over: the first lookup since wasn't turned down.
    Resumed,
    /// A game's prices couldn't be looked up, by app ID: the market's
    /// answer couldn't be used, for this reason. They're tried again a day
    /// later.
    Failed { app_id: AppId, why: String },
    /// The market couldn't be asked, or didn't answer, for this reason: it's
    /// asked again after `wait`, at `retry_at`, and nothing is taken as
    /// failed meanwhile.
    Unanswered {
        why: String,
        wait: Duration,
        retry_at: DateTime<Utc>,
    },
}

//! The background pricing: the games to price, in order, each as it falls
//! due, waiting out Steam's pause and a market that couldn't be asked.

use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use game::AppId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    Clock, MarketPause, PriceBook, PriceEvent, PriceEventKind, PriceRepository, SetPrices,
    pricing::{Priced, price_set},
    rules::{TICK, UNANSWERED_FIRST, UNANSWERED_LONGEST, between, later},
};

/// The background pricing: what's wanted, in order, as it falls due.
pub(crate) struct Watcher {
    repo: Arc<dyn PriceRepository>,
    clock: Clock,
    events: mpsc::Sender<PriceEvent>,
}

impl Watcher {
    pub(crate) fn new(
        repo: Arc<dyn PriceRepository>,
        clock: Clock,
        events: mpsc::Sender<PriceEvent>,
    ) -> Self {
        Self {
            repo,
            clock,
            events,
        }
    }

    pub(crate) async fn run(&self, token: &CancellationToken) {
        // Sets looked up since everything wanted was last priced.
        let mut looked_up = 0;
        // Steam's pause, from when it's first told until Steam answers.
        let mut paused = None;
        // How long the last wait for a market that couldn't be asked was.
        let mut unanswered: Option<Duration> = None;
        loop {
            if token.is_cancelled() {
                return;
            }
            let wanted = unique(self.repo.wanted());
            let book = self.repo.book();
            let now = (self.clock)();
            let due = wanted.iter().copied().find(|&id| is_due(&book, id, now));
            let Some(app_id) = due else {
                if looked_up > 0 {
                    looked_up = 0;
                    self.all_priced(&wanted, &book, now);
                }
                let next = wanted
                    .iter()
                    .filter_map(|id| book.sets.get(id))
                    .map(SetPrices::due_at)
                    .min();
                let wait = next.map_or(TICK, |next| between(now, next).min(TICK));
                if !sleep(wait, token).await {
                    return;
                }
                continue;
            };
            let priced = tokio::select! {
                _ = token.cancelled() => return,
                priced = price_set(&*self.repo, app_id, &self.clock) => priced,
            };
            if let Priced::Paused(pause) = priced {
                // The same pause, told again, is nothing new.
                if paused != Some(pause) {
                    let again = paused.is_some();
                    self.report(PriceEventKind::Paused(pause), paused_line(pause, again));
                }
                paused = Some(pause);
                if !self.wait_out(pause, token).await {
                    return;
                }
                continue;
            }
            // Nothing was kept: the same set is due again after the wait.
            if let Priced::Unanswered(why) = priced {
                let wait = unanswered.map_or(UNANSWERED_FIRST, |w| (w * 2).min(UNANSWERED_LONGEST));
                unanswered = Some(wait);
                self.report(
                    PriceEventKind::Unanswered {
                        retry_at: later((self.clock)(), wait),
                    },
                    format!(
                        "Couldn't ask the market for prices: {why}. Asking again in {}.",
                        minutes(wait)
                    ),
                );
                if !sleep(wait, token).await {
                    return;
                }
                continue;
            }
            unanswered = None;
            if paused.take().is_some() {
                self.report(
                    PriceEventKind::Resumed,
                    "Steam's pause on price lookups is over: they carry on.".into(),
                );
            }
            if let Priced::Failed(why) = priced {
                self.report(
                    PriceEventKind::Failed(app_id),
                    format!(
                        "Couldn't look up the prices of app {app_id}'s cards: {why}. \
                         They're tried again in 24 hours."
                    ),
                );
            }
            looked_up += 1;
        }
    }

    /// Everything wanted is priced: says so, and when the next round is.
    fn all_priced(&self, wanted: &[AppId], book: &PriceBook, now: DateTime<Utc>) {
        let games = match wanted.len() {
            1 => "the 1 game".to_owned(),
            n => format!("all {n} games"),
        };
        let next_round = wanted
            .iter()
            .filter_map(|id| book.sets.get(id))
            .map(SetPrices::due_at)
            .min();
        let next = next_round.map_or_else(String::new, |next| {
            format!("; the next round is in {}", in_words(between(now, next)))
        });
        self.report(
            PriceEventKind::AllPriced {
                games: wanted.len(),
                next_round,
            },
            format!("Prices: {games} looked up{next}."),
        );
    }

    /// Waits until Steam's pause ends, a tick at a time, so a clock that
    /// jumped (a computer that slept) is noticed. False when stopped.
    async fn wait_out(&self, pause: MarketPause, token: &CancellationToken) -> bool {
        // Over already by this clock, but not by the market's: a tick, and
        // then ask again, rather than asking again and again.
        if between((self.clock)(), pause.until).is_zero() {
            return sleep(TICK, token).await;
        }
        loop {
            let left = between((self.clock)(), pause.until);
            if left.is_zero() {
                return true;
            }
            if !sleep(left.min(TICK), token).await {
                return false;
            }
        }
    }

    // try_send keeps events in order without blocking the watcher; the UI
    // drains continuously, so the buffer only fills if it has gone away.
    fn report(&self, kind: PriceEventKind, message: String) {
        let _ = self.events.try_send(PriceEvent { kind, message });
    }
}

/// Whether a game's set is due a lookup: never looked up, 6 hours old, or a
/// day after a failed lookup.
fn is_due(book: &PriceBook, app_id: AppId, now: DateTime<Utc>) -> bool {
    book.sets.get(&app_id).is_none_or(|set| set.due_at() <= now)
}

/// Each game once, as first listed.
fn unique(app_ids: Vec<AppId>) -> Vec<AppId> {
    let mut seen = HashSet::new();
    app_ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

/// Sleeps for `d`; false when stopped meanwhile.
async fn sleep(d: Duration, token: &CancellationToken) -> bool {
    tokio::select! {
        _ = token.cancelled() => false,
        _ = tokio::time::sleep(d) => true,
    }
}

/// The log line for a pause: the first, or one after another.
fn paused_line(pause: MarketPause, again: bool) -> String {
    let wait = minutes(pause.step);
    if again {
        format!("Steam turned down price lookups again: they wait {wait} now. Farming carries on.")
    } else {
        format!("Steam turned down a price lookup: lookups wait {wait}. Farming carries on.")
    }
}

/// "10 minutes", "an hour".
fn minutes(d: Duration) -> String {
    match d.as_secs() / 60 {
        60 => "an hour".into(),
        1 => "a minute".into(),
        m => format!("{m} minutes"),
    }
}

/// A duration in two units at most, as the screens write them: "<1m",
/// "38m", "5h 58m", "4d 21h".
fn in_words(d: Duration) -> String {
    let minutes = d.as_secs() / 60;
    let (days, hours, minutes) = (minutes / (24 * 60), minutes / 60 % 24, minutes % 60);
    match (days, hours, minutes) {
        (0, 0, 0) => "<1m".into(),
        (0, 0, m) => format!("{m}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    #[test]
    fn durations_read_as_the_screens_write_them() {
        assert_eq!(in_words(Duration::from_secs(20)), "<1m");
        assert_eq!(in_words(38 * MINUTE), "38m");
        assert_eq!(in_words(6 * 60 * MINUTE), "6h");
        assert_eq!(in_words(358 * MINUTE), "5h 58m");
        assert_eq!(in_words((4 * 24 + 21) * 60 * MINUTE + 5 * MINUTE), "4d 21h");
        assert_eq!(in_words(2 * 24 * 60 * MINUTE), "2d");
        assert_eq!(minutes(10 * MINUTE), "10 minutes");
        assert_eq!(minutes(60 * MINUTE), "an hour");
    }

    #[test]
    fn a_game_wanted_twice_is_priced_once() {
        assert_eq!(
            unique([620, 440, 620, 730, 440].map(AppId).to_vec()),
            [620, 440, 730].map(AppId)
        );
    }
}

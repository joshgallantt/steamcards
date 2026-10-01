//! The background pricing: the games to price, in order, each as it falls
//! due, waiting out Steam's pause and a market that couldn't be asked.

use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use game::AppId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    CardPriceRepository, Clock, MarketPause, PriceBook, PriceEvent, SetPrices,
    model::rules::{TICK, UNANSWERED_FIRST, UNANSWERED_LONGEST, between, later},
};

use super::pricing::{Priced, price_set};

/// The background pricing: what's wanted, in order, as it falls due.
pub(crate) struct PriceWatcher {
    repo: Arc<dyn CardPriceRepository>,
    clock: Clock,
    events: mpsc::Sender<PriceEvent>,
}

impl PriceWatcher {
    pub(crate) fn new(
        repo: Arc<dyn CardPriceRepository>,
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
                    self.report(PriceEvent::Paused { pause, again });
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
                self.report(PriceEvent::Unanswered {
                    why,
                    wait,
                    retry_at: later((self.clock)(), wait),
                });
                if !sleep(wait, token).await {
                    return;
                }
                continue;
            }
            unanswered = None;
            if paused.take().is_some() {
                self.report(PriceEvent::Resumed);
            }
            if let Priced::Failed(why) = priced {
                self.report(PriceEvent::Failed { app_id, why });
            }
            looked_up += 1;
        }
    }

    /// Everything wanted is priced: says so, and when the next round is.
    fn all_priced(&self, wanted: &[AppId], book: &PriceBook, now: DateTime<Utc>) {
        let next_round = wanted
            .iter()
            .filter_map(|id| book.sets.get(id))
            .map(SetPrices::due_at)
            .min();
        self.report(PriceEvent::AllPriced {
            games: wanted.len(),
            next_round,
            at: now,
        });
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
    fn report(&self, event: PriceEvent) {
        let _ = self.events.try_send(event);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_wanted_twice_is_priced_once() {
        assert_eq!(
            unique([620, 440, 620, 730, 440].map(AppId).to_vec()),
            [620, 440, 730].map(AppId)
        );
    }
}

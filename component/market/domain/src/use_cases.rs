//! One function per thing asked of the market. Each use case is a value: its
//! type says what it takes and gives, and a constructor builds the real one
//! over the repository.
//!
//! The market decides what to price, and in what order. How fast requests
//! go is Steam's business: every lookup waits its turn in the data layer's
//! one market queue. An answer that can't be used is tried again a day
//! later; a market that couldn't be asked at all, soon, with nothing taken
//! as failed.

use std::{collections::HashSet, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    Basis, Lookup, MarketError, MarketEvent, MarketEventKind, MarketPause, MarketRepository,
    MarketSettings, Offers, Price, PriceBook, SetPrices, Wallet,
    rules::{
        ASKED_AGAIN_AFTER, OFFERS_FRESH_FOR, RETRY_FAILED, TICK, UNANSWERED_FIRST,
        UNANSWERED_LONGEST, between, later,
    },
};

/// What time it is. The real clock is the system's; a test's can move with
/// tokio's paused time, so hours of pricing pass in moments.
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// Everything priced so far.
pub type GetPrices = Arc<dyn Fn() -> Arc<PriceBook> + Send + Sync>;

/// Says which games to price, most urgent first: the game being farmed,
/// then games with cards this session, then the rest in farm order.
pub type WantPrices = Arc<dyn Fn(Vec<u32>) + Send + Sync>;

/// Prices the wanted games' sets, normal cards and foils, until the token is
/// cancelled, reporting on the channel: each set once, then again once its
/// prices are 6 hours old, in the order wanted. While Steam has paused
/// lookups, it waits. Runs detached; the handle resolves once it has
/// stopped.
pub type WatchPrices =
    Arc<dyn Fn(CancellationToken, mpsc::Sender<MarketEvent>) -> JoinHandle<()> + Send + Sync>;

/// Prices a game's set again in the background, if its prices are over an
/// hour old: one of its cards just dropped, or the user asked. Errs when
/// Steam has paused lookups, or the market couldn't be asked.
pub type RefreshPrices = Arc<dyn Fn(u32) -> JoinHandle<Result<(), MarketError>> + Send + Sync>;

/// Looks up the order books of these cards, by market hash name, in the
/// background: their best offers, which only the instant basis uses. A card
/// whose order book was looked up under half an hour ago isn't looked up
/// again, whatever it said. Errs when Steam has paused lookups, or the
/// market couldn't be asked; the rest wait for the next ask.
pub type PriceOffers =
    Arc<dyn Fn(Vec<String>) -> JoinHandle<Result<(), MarketError>> + Send + Sync>;

/// The account's wallet: its currency, and the fees Steam takes. `None`
/// until Steam has said.
pub type GetWallet = Arc<dyn Fn() -> Option<Wallet> + Send + Sync>;

/// The market's settings: the value basis.
pub type GetMarketSettings = Arc<dyn Fn() -> MarketSettings + Send + Sync>;

/// Values money on this basis from now on.
pub type SetBasis = Arc<dyn Fn(Basis) -> Result<(), MarketError> + Send + Sync>;

/// The system's clock.
pub fn system_clock() -> Clock {
    Arc::new(Utc::now)
}

pub fn get_prices(repo: Arc<dyn MarketRepository>) -> GetPrices {
    Arc::new(move || repo.book())
}

pub fn want_prices(repo: Arc<dyn MarketRepository>) -> WantPrices {
    Arc::new(move |app_ids| repo.want(app_ids))
}

pub fn watch_prices(repo: Arc<dyn MarketRepository>, clock: Clock) -> WatchPrices {
    Arc::new(move |token, events| {
        let watcher = Watcher {
            repo: Arc::clone(&repo),
            clock: Arc::clone(&clock),
            events,
        };
        tokio::spawn(async move { watcher.run(&token).await })
    })
}

pub fn refresh_prices(repo: Arc<dyn MarketRepository>, clock: Clock) -> RefreshPrices {
    Arc::new(move |app_id| {
        let (repo, clock) = (Arc::clone(&repo), Arc::clone(&clock));
        tokio::spawn(async move {
            let recent = repo
                .book()
                .sets
                .get(&app_id)
                .is_some_and(|set| between(set.fetched_at, clock()) <= ASKED_AGAIN_AFTER);
            if recent {
                return Ok(());
            }
            match price_set(&*repo, app_id, &clock).await {
                Priced::Paused(pause) => Err(MarketError::Paused(pause)),
                Priced::Unanswered(_) => Err(MarketError::Unanswered),
                Priced::Done | Priced::Failed(_) => Ok(()),
            }
        })
    })
}

pub fn price_offers(repo: Arc<dyn MarketRepository>, clock: Clock) -> PriceOffers {
    Arc::new(move |hashes| {
        let (repo, clock) = (Arc::clone(&repo), Arc::clone(&clock));
        tokio::spawn(async move {
            let mut seen = HashSet::new();
            for hash in hashes.into_iter().filter(|h| seen.insert(h.clone())) {
                let book = repo.book();
                if is_recent(book.offers.get(&hash), clock()) {
                    continue;
                }
                let price = match repo.look_up_offers(&hash).await {
                    Ok(Lookup::Found(price)) => price,
                    Ok(Lookup::Paused(pause)) => return Err(MarketError::Paused(pause)),
                    Ok(Lookup::Unanswered(_)) => return Err(MarketError::Unanswered),
                    Err(_) => Price::failed(clock()),
                };
                let looked_up_at = clock();
                repo.keep_offers(
                    &hash,
                    Offers {
                        price,
                        looked_up_at,
                    },
                );
            }
            Ok(())
        })
    })
}

pub fn get_wallet(repo: Arc<dyn MarketRepository>) -> GetWallet {
    Arc::new(move || repo.wallet())
}

pub fn get_market_settings(repo: Arc<dyn MarketRepository>) -> GetMarketSettings {
    Arc::new(move || repo.settings())
}

pub fn set_basis(repo: Arc<dyn MarketRepository>) -> SetBasis {
    Arc::new(move |basis| {
        let mut settings = repo.settings();
        settings.basis = basis;
        repo.save_settings(settings)
            .map_err(|_| MarketError::Unavailable)
    })
}

/// An order book looked up under half an hour ago, or a failed one not due
/// again yet.
fn is_recent(offers: Option<&Offers>, now: DateTime<Utc>) -> bool {
    match offers {
        Some(Offers {
            price: Price::Failed { retry_at },
            ..
        }) => *retry_at > now,
        Some(offers) => between(offers.looked_up_at, now) < OFFERS_FRESH_FOR,
        None => false,
    }
}

/// How pricing a set went.
enum Priced {
    Done,
    Paused(MarketPause),
    /// Steam's answer couldn't be used, for this reason.
    Failed(String),
    /// The market couldn't be asked, or didn't answer, for this reason:
    /// nothing was kept.
    Unanswered(String),
}

/// Looks a game's set up afresh, normal cards then foils, and keeps it. A
/// set's prices come from one lookup at one time: when either half's answer
/// can't be used, it keeps the prices it had, and is tried again a day
/// later. When the market couldn't be asked, it's left as it was.
async fn price_set(repo: &dyn MarketRepository, app_id: u32, clock: &Clock) -> Priced {
    let mut borders = [Vec::new(), Vec::new()];
    for (foil, cards) in [false, true].into_iter().zip(&mut borders) {
        match repo.look_up_set(app_id, foil).await {
            Ok(Lookup::Found(found)) => *cards = found,
            Ok(Lookup::Paused(pause)) => return Priced::Paused(pause),
            Ok(Lookup::Unanswered(why)) => return Priced::Unanswered(why),
            Err(e) => {
                let now = clock();
                let failed = match repo.book().sets.get(&app_id) {
                    Some(before) => SetPrices {
                        retry_at: Some(later(now, RETRY_FAILED)),
                        ..before.clone()
                    },
                    None => SetPrices::failed(app_id, now),
                };
                repo.keep_set(failed);
                return Priced::Failed(e.to_string());
            }
        }
    }
    let [normal, foil] = borders;
    repo.keep_set(SetPrices {
        app_id,
        normal,
        foil,
        fetched_at: clock(),
        retry_at: None,
    });
    Priced::Done
}

/// The background pricing: what's wanted, in order, as it falls due.
struct Watcher {
    repo: Arc<dyn MarketRepository>,
    clock: Clock,
    events: mpsc::Sender<MarketEvent>,
}

impl Watcher {
    async fn run(&self, token: &CancellationToken) {
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
                    self.report(MarketEventKind::Paused(pause), paused_line(pause, again));
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
                    MarketEventKind::Unanswered {
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
                    MarketEventKind::Resumed,
                    "Steam's pause on price lookups is over: they carry on.".into(),
                );
            }
            if let Priced::Failed(why) = priced {
                self.report(
                    MarketEventKind::Failed(app_id),
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
    fn all_priced(&self, wanted: &[u32], book: &PriceBook, now: DateTime<Utc>) {
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
            MarketEventKind::AllPriced {
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
    fn report(&self, kind: MarketEventKind, message: String) {
        let _ = self.events.try_send(MarketEvent { kind, message });
    }
}

/// Whether a game's set is due a lookup: never looked up, 6 hours old, or a
/// day after a failed lookup.
fn is_due(book: &PriceBook, app_id: u32, now: DateTime<Utc>) -> bool {
    book.sets.get(&app_id).is_none_or(|set| set.due_at() <= now)
}

/// Each game once, as first listed.
fn unique(app_ids: Vec<u32>) -> Vec<u32> {
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
        assert_eq!(unique(vec![620, 440, 620, 730, 440]), [620, 440, 730]);
    }
}

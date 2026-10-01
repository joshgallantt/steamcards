use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use card::CardKind;
use chrono::{DateTime, TimeDelta, Utc};
use game::AppId;

use crate::{
    Clock, Lookup, MarketPause, PriceBook, PriceRepository, PricedCard, SetPrices, Wallet,
    test_support::{SetLookup, listing, pounds, priced_card},
};

/// What a set lookup finds, by game and kind: each card's name and list
/// price.
type Listings = HashMap<(u32, CardKind), Vec<(String, i64)>>;

/// The market in memory: the prices it lists, and the book and the wallet
/// as a real one keeps them. Its lookups behave as Steam's
/// market queue does: while Steam has paused lookups, one is turned away
/// without asking; once the pause is over, one goes, and if Steam turns
/// that down too, the pause doubles, to an hour at most.
pub struct FakePriceRepository {
    clock: Clock,
    book: Mutex<Arc<PriceBook>>,
    /// A game that isn't here has nothing listed.
    listed: Mutex<Listings>,
    /// Games whose lookups get an answer that can't be used.
    failing: Mutex<HashSet<u32>>,
    /// How many more lookups can't reach the market, and why.
    unreachable: Mutex<Option<(u32, String)>>,
    pause: Mutex<Option<MarketPause>>,
    /// How many more lookups Steam turns down.
    turn_down: AtomicU32,
    /// How many more lookups the queue turns away with its pause, whatever
    /// the clock says.
    held: AtomicU32,
    wallet: Mutex<Option<Wallet>>,
    wanted: Mutex<Vec<AppId>>,
    set_lookups: Mutex<Vec<SetLookup>>,
    /// Lookups Steam turned down, and when.
    turned_down: Mutex<Vec<DateTime<Utc>>>,
}

impl FakePriceRepository {
    /// Nothing listed and nothing priced, in a wallet in pounds, on `clock`.
    pub fn new(clock: Clock) -> Self {
        Self {
            clock,
            book: Mutex::default(),
            listed: Mutex::default(),
            failing: Mutex::default(),
            unreachable: Mutex::default(),
            pause: Mutex::default(),
            turn_down: AtomicU32::new(0),
            held: AtomicU32::new(0),
            wallet: Mutex::new(Some(pounds())),
            wanted: Mutex::default(),
            set_lookups: Mutex::default(),
            turned_down: Mutex::default(),
        }
    }

    /// The market lists `app_id`'s cards at these prices in pence: its
    /// normal cards, and its foils.
    pub fn lists(&self, app_id: u32, normal: &[(&str, i64)], foil: &[(&str, i64)]) {
        let own = |cards: &[(&str, i64)]| {
            cards
                .iter()
                .map(|&(name, ask)| (name.to_owned(), ask))
                .collect()
        };
        let mut listed = self.listed.lock().unwrap();
        listed.insert((app_id, CardKind::Normal), own(normal));
        listed.insert((app_id, CardKind::Foil), own(foil));
    }

    /// Lookups of `app_id`'s set get an answer that can't be used.
    pub fn fails(&self, app_id: u32) {
        self.failing.lock().unwrap().insert(app_id);
    }

    /// The next `times` lookups can't reach the market, for this reason: no
    /// sign-in, no network.
    pub fn cant_be_asked(&self, times: u32, why: &str) {
        *self.unreachable.lock().unwrap() = Some((times, why.to_owned()));
    }

    /// Steam has paused lookups, as a real market kept it from before.
    pub fn paused(&self, pause: MarketPause) {
        *self.pause.lock().unwrap() = Some(pause);
    }

    /// Steam turns down the next `times` lookups that reach it.
    pub fn turns_down(&self, times: u32) {
        self.turn_down.store(times, Ordering::Relaxed);
    }

    /// The queue turns the next `times` lookups away with its pause, even
    /// once the clock says it's over: its own clock didn't count the time
    /// the computer slept.
    pub fn holds_pause(&self, pause: MarketPause, times: u32) {
        self.paused(pause);
        self.held.store(times, Ordering::Relaxed);
    }

    /// The book holds these sets already, as if kept from before.
    pub fn knows(&self, sets: Vec<SetPrices>) {
        let mut book = self.book.lock().unwrap();
        let mut next = PriceBook::clone(&book);
        for set in sets {
            next.sets.insert(set.app_id, set);
        }
        *book = Arc::new(next);
    }

    /// Steam hasn't said what the wallet is yet, or now it has.
    pub fn wallet_is(&self, wallet: Option<Wallet>) {
        *self.wallet.lock().unwrap() = wallet;
    }

    /// Every set lookup Steam answered, in order.
    pub fn set_lookups(&self) -> Vec<SetLookup> {
        self.set_lookups.lock().unwrap().clone()
    }

    /// Just the games and kinds of each set lookup.
    pub fn looked_up(&self) -> Vec<(u32, CardKind)> {
        self.set_lookups()
            .into_iter()
            .map(|l| (l.app_id, l.kind))
            .collect()
    }

    /// When each lookup Steam turned down went.
    pub fn turned_down(&self) -> Vec<DateTime<Utc>> {
        self.turned_down.lock().unwrap().clone()
    }

    /// Why a lookup can't reach the market now, if it can't.
    fn unreachable(&self) -> Option<String> {
        let mut unreachable = self.unreachable.lock().unwrap();
        let (left, why) = unreachable.as_mut().filter(|(left, _)| *left > 0)?;
        *left -= 1;
        Some(why.clone())
    }

    /// Whether a lookup may go now, as Steam's market queue decides: the
    /// pause if not.
    fn queue(&self) -> Option<MarketPause> {
        let now = (self.clock)();
        let mut pause = self.pause.lock().unwrap();
        let held = self
            .held
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
            .is_ok();
        if let Some(p) = *pause
            && (held || now < p.until)
        {
            return Some(p);
        }
        let turned_down = self
            .turn_down
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
            .is_ok();
        if !turned_down {
            *pause = None;
            return None;
        }
        self.turned_down.lock().unwrap().push(now);
        let step = pause.map_or(Duration::from_secs(10 * 60), |p| {
            (p.step * 2).min(Duration::from_secs(60 * 60))
        });
        let p = MarketPause {
            until: now + TimeDelta::from_std(step).unwrap_or_default(),
            step,
        };
        *pause = Some(p);
        Some(p)
    }
}

#[async_trait]
impl PriceRepository for FakePriceRepository {
    fn book(&self) -> Arc<PriceBook> {
        Arc::clone(&self.book.lock().unwrap())
    }

    fn keep_set(&self, set: SetPrices) {
        self.knows(vec![set]);
    }

    async fn look_up_set(
        &self,
        app_id: AppId,
        kind: CardKind,
    ) -> anyhow::Result<Lookup<Vec<PricedCard>>> {
        let app_id = app_id.0;
        if let Some(why) = self.unreachable() {
            return Ok(Lookup::Unanswered(why));
        }
        if let Some(pause) = self.queue() {
            return Ok(Lookup::Paused(pause));
        }
        let now = (self.clock)();
        self.set_lookups.lock().unwrap().push(SetLookup {
            app_id,
            kind,
            at: now,
        });
        if self.failing.lock().unwrap().contains(&app_id) {
            anyhow::bail!("steamcommunity.com's market said 502 Bad Gateway, twice");
        }
        let listed = self
            .listed
            .lock()
            .unwrap()
            .get(&(app_id, kind))
            .cloned()
            .unwrap_or_default();
        Ok(Lookup::Found(
            listed
                .iter()
                .map(|(name, ask)| priced_card(app_id, name, kind, listing(*ask, 50, now)))
                .collect(),
        ))
    }

    fn wallet(&self) -> Option<Wallet> {
        *self.wallet.lock().unwrap()
    }

    fn wanted(&self) -> Vec<AppId> {
        self.wanted.lock().unwrap().clone()
    }

    fn want(&self, app_ids: Vec<AppId>) {
        *self.wanted.lock().unwrap() = app_ids;
    }
}

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use debug_log::DebugLog;
use price::{
    Lookup, Offers, Price, PriceBook, PriceRepository, PriceSettings, PricedCard, SetPrices, Wallet,
};
use steam_library::AppId;

use crate::{MarketClient, PriceStore};

/// How long a set's prices are kept once looked up: shown dim with their
/// age after 6 hours, and gone after a week (research §1.3's cache).
const KEPT_FOR: TimeDelta = TimeDelta::days(7);

/// The prices looked up through the client, the book of them in memory,
/// and what's kept of them in the store.
pub struct DefaultPriceRepository {
    client: Arc<dyn MarketClient>,
    store: Arc<dyn PriceStore>,
    log: DebugLog,
    book: Mutex<Arc<PriceBook>>,
    wanted: Mutex<Vec<AppId>>,
}

impl DefaultPriceRepository {
    /// Takes up what was kept from before: the prices under a week old, and
    /// Steam's pause, if there was one.
    pub fn new(client: Arc<dyn MarketClient>, store: Arc<dyn PriceStore>, log: DebugLog) -> Self {
        if let Some(pause) = store.pause() {
            client.resume(pause);
        }
        let now = Utc::now();
        let mut book = PriceBook::default();
        for set in store.sets() {
            if is_kept(&set, now) {
                book.sets.insert(set.app_id, set);
            }
        }
        Self {
            client,
            store,
            log,
            book: Mutex::new(Arc::new(book)),
            wanted: Mutex::default(),
        }
    }

    /// Keeps Steam's pause when it has changed, so it outlasts a restart.
    fn keep_pause(&self) {
        if let Err(e) = self.store.save_pause(self.client.pause()) {
            self.log
                .line(&format!("couldn't keep Steam's pause on the market: {e}"));
        }
    }
}

#[async_trait]
impl PriceRepository for DefaultPriceRepository {
    fn book(&self) -> Arc<PriceBook> {
        Arc::clone(&self.book.lock().unwrap())
    }

    fn keep_set(&self, set: SetPrices) {
        let now = Utc::now();
        let book = {
            let mut book = self.book.lock().unwrap();
            let mut next = PriceBook::clone(&book);
            next.sets.insert(set.app_id, set);
            next.sets.retain(|_, set| is_kept(set, now));
            *book = Arc::new(next);
            Arc::clone(&book)
        };
        if let Err(e) = self.store.save_sets(&book) {
            // Only a restart would miss them: they'd be looked up again.
            self.log.line(&format!("couldn't keep prices on disk: {e}"));
        }
    }

    fn keep_offers(&self, market_hash_name: &str, offers: Offers) {
        let mut book = self.book.lock().unwrap();
        let mut next = PriceBook::clone(&book);
        next.offers.insert(market_hash_name.to_owned(), offers);
        *book = Arc::new(next);
    }

    async fn look_up_set(
        &self,
        app_id: AppId,
        foil: bool,
    ) -> anyhow::Result<Lookup<Vec<PricedCard>>> {
        let answer = self.client.look_up_set(app_id, foil).await;
        self.keep_pause();
        answer
    }

    async fn look_up_offers(&self, market_hash_name: &str) -> anyhow::Result<Lookup<Price>> {
        let answer = self.client.look_up_offers(market_hash_name).await;
        self.keep_pause();
        answer
    }

    fn wallet(&self) -> Option<Wallet> {
        self.client.wallet()
    }

    fn settings(&self) -> PriceSettings {
        self.store.settings()
    }

    fn save_settings(&self, settings: PriceSettings) -> anyhow::Result<()> {
        self.store.save_settings(settings)
    }

    fn wanted(&self) -> Vec<AppId> {
        self.wanted.lock().unwrap().clone()
    }

    fn want(&self, app_ids: Vec<AppId>) {
        *self.wanted.lock().unwrap() = app_ids;
    }
}

fn is_kept(set: &SetPrices, now: DateTime<Utc>) -> bool {
    now - set.fetched_at <= KEPT_FOR
}

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;
use game::{AppId, Game, SteamLibrary};
use tokio::sync::Notify;

use crate::{AssetId, CardAsset, CardRepository, CardSet, GameCards, NewItem};

/// Games' cards held in memory: each game's card page, its foils, the
/// copies of cards the account holds, and what Steam announces is new.
#[derive(Default)]
pub struct FakeCardRepository {
    /// The games whose card pages can be looked at.
    pub library: Mutex<SteamLibrary>,
    /// Each game's set, as its card page shows it. A game of the library
    /// that isn't here shows none.
    pub sets: Mutex<HashMap<AppId, CardSet>>,
    /// Each game's foils, and how many of each the account has. A game of
    /// the library that isn't here has none yet.
    pub foils: Mutex<HashMap<AppId, CardSet>>,
    /// The copies of cards the account holds. Anything else it holds isn't
    /// a card, so it's never described.
    pub assets: Mutex<Vec<CardAsset>>,
    /// Everything fails, as if Steam were down.
    pub down: AtomicBool,
    /// What Steam has announced is new, and nobody has heard yet.
    announced: Mutex<VecDeque<Vec<NewItem>>>,
    told: Notify,
}

impl FakeCardRepository {
    pub fn with(games: Vec<Game>) -> Self {
        Self {
            library: Mutex::new(SteamLibrary::new(games)),
            ..Default::default()
        }
    }

    /// Steam announces these items as new: none, when it only counts more.
    pub fn announce(&self, items: Vec<NewItem>) {
        self.announced.lock().unwrap().push_back(items);
        self.told.notify_one();
    }

    fn game(&self, app_id: AppId) -> anyhow::Result<Game> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("steamcommunity.com didn't answer");
        }
        self.library
            .lock()
            .unwrap()
            .game(app_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no game {app_id}"))
    }
}

#[async_trait]
impl CardRepository for FakeCardRepository {
    async fn game_cards(&self, app_id: AppId) -> anyhow::Result<GameCards> {
        let game = self.game(app_id)?;
        let set = self
            .sets
            .lock()
            .unwrap()
            .get(&app_id)
            .cloned()
            .unwrap_or_default();
        Ok(GameCards { game, set })
    }

    async fn foils(&self, app_id: AppId) -> anyhow::Result<CardSet> {
        self.game(app_id)?;
        Ok(self
            .foils
            .lock()
            .unwrap()
            .get(&app_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("Steam didn't answer in time");
        }
        Ok(among(&self.assets.lock().unwrap(), asset_ids))
    }

    async fn next_new_items(&self) -> Vec<NewItem> {
        loop {
            if let Some(items) = self.announced.lock().unwrap().pop_front() {
                return items;
            }
            self.told.notified().await;
        }
    }
}

/// The cards in `held` with these asset IDs, each once, in the order asked.
fn among(held: &[CardAsset], asset_ids: &[AssetId]) -> Vec<CardAsset> {
    let mut seen = HashSet::new();
    asset_ids
        .iter()
        .filter(|id| seen.insert(**id))
        .filter_map(|id| held.iter().find(|a| a.asset_id == *id).cloned())
        .collect()
}

//! Doubles for other crates' tests. A double stands in for the contract, so
//! no test needs Steam.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;

use crate::{
    Card, CardAsset, CardDrops, DescribeCards, Game, LibraryError, LibraryRepository, LookAtGame,
    ReadLibrary, SteamLibrary,
};

/// A game with `received` and `remaining` card drops and `hours` played,
/// named "Game <app ID>" unless named after.
pub fn game(app_id: u32, hours: f64, received: u32, remaining: u32) -> Game {
    Game {
        app_id,
        name: format!("Game {app_id}"),
        hours,
        drops: CardDrops {
            received,
            remaining,
        },
        badge_level: 0,
        cards: Vec::new(),
    }
}

/// A copy of `name` from `app_id`'s set, held as `asset_id`: not a foil,
/// marketable and tradable, with the market hash name Steam would give it.
pub fn card_asset(asset_id: u64, app_id: u32, name: &str) -> CardAsset {
    CardAsset {
        asset_id,
        app_id,
        name: name.to_owned(),
        market_hash_name: format!("{app_id}-{name}"),
        foil: false,
        marketable: true,
        tradable: true,
    }
}

/// Answers every read with `library`.
pub fn fixed_library(library: SteamLibrary) -> ReadLibrary {
    Arc::new(move || {
        let library = library.clone();
        tokio::spawn(async move { Ok(library) })
    })
}

/// Fails every read, saying `why`.
pub fn unreadable_library(why: &str) -> ReadLibrary {
    let why = why.to_owned();
    Arc::new(move || {
        let why = why.clone();
        tokio::spawn(async move { Err(LibraryError::Unavailable(why)) })
    })
}

/// Looks at nothing: every game is as the library last said.
pub fn no_look(library: SteamLibrary) -> LookAtGame {
    Arc::new(move |app_id| {
        let found = library.game(app_id).cloned();
        tokio::spawn(async move {
            found.ok_or_else(|| LibraryError::Unavailable(format!("no game {app_id}")))
        })
    })
}

/// Describes items as `held` says: an ID among them is that card, and any
/// other is unknown.
pub fn fixed_cards(held: Vec<CardAsset>) -> DescribeCards {
    Arc::new(move |asset_ids| {
        let found = among(&held, &asset_ids);
        tokio::spawn(async move { Ok(found) })
    })
}

/// A library held in memory, and the cards the account holds.
#[derive(Default)]
pub struct InMemoryLibraryRepository {
    pub library: Mutex<SteamLibrary>,
    /// The copies of cards the account holds. Anything else it holds isn't
    /// a card, so it's never described.
    pub assets: Mutex<Vec<CardAsset>>,
    /// Each game's foils, and how many of each the account has. A game of
    /// the library that isn't here has none yet.
    pub foils: Mutex<HashMap<u32, Vec<Card>>>,
    /// Everything fails, as if Steam were down.
    pub down: AtomicBool,
}

impl InMemoryLibraryRepository {
    pub fn with(games: Vec<Game>) -> Self {
        Self {
            library: Mutex::new(SteamLibrary::new(games)),
            assets: Mutex::default(),
            foils: Mutex::default(),
            down: AtomicBool::new(false),
        }
    }

    /// Holding `assets` as well.
    pub fn holding(self, assets: Vec<CardAsset>) -> Self {
        *self.assets.lock().unwrap() = assets;
        self
    }
}

#[async_trait]
impl LibraryRepository for InMemoryLibraryRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("steamcommunity.com didn't answer");
        }
        Ok(self.library.lock().unwrap().clone())
    }

    async fn game(&self, app_id: u32) -> anyhow::Result<Game> {
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

    async fn foils(&self, app_id: u32) -> anyhow::Result<Vec<Card>> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("steamcommunity.com didn't answer");
        }
        if self.library.lock().unwrap().game(app_id).is_none() {
            anyhow::bail!("no game {app_id}");
        }
        Ok(self
            .foils
            .lock()
            .unwrap()
            .get(&app_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn describe(&self, asset_ids: &[u64]) -> anyhow::Result<Vec<CardAsset>> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("Steam didn't answer in time");
        }
        Ok(among(&self.assets.lock().unwrap(), asset_ids))
    }
}

/// The cards in `held` with these asset IDs, each once, in the order asked.
fn among(held: &[CardAsset], asset_ids: &[u64]) -> Vec<CardAsset> {
    let mut seen = HashSet::new();
    asset_ids
        .iter()
        .filter(|id| seen.insert(**id))
        .filter_map(|id| held.iter().find(|a| a.asset_id == *id).cloned())
        .collect()
}

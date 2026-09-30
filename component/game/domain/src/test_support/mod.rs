//! Doubles for other crates' tests. A double stands in for the contract, so
//! no test needs Steam.

mod spies;

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;

use crate::{AppId, CardDrops, Game, GameRepository, SteamLibrary};

pub use spies::SpyReadLibraryUseCase;

/// A game with `received` and `remaining` card drops and `hours` played,
/// named "Game <app ID>" unless named after.
pub fn game(app_id: u32, hours: f64, received: u32, remaining: u32) -> Game {
    Game {
        app_id: AppId(app_id),
        name: format!("Game {app_id}"),
        hours,
        drops: CardDrops {
            received,
            remaining,
        },
        badge_level: 0,
    }
}

/// A library held in memory.
#[derive(Default)]
pub struct InMemoryGameRepository {
    pub library: Mutex<SteamLibrary>,
    /// Everything fails, as if Steam were down.
    pub down: AtomicBool,
}

impl InMemoryGameRepository {
    pub fn with(games: Vec<Game>) -> Self {
        Self {
            library: Mutex::new(SteamLibrary::new(games)),
            down: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl GameRepository for InMemoryGameRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("steamcommunity.com didn't answer");
        }
        Ok(self.library.lock().unwrap().clone())
    }
}

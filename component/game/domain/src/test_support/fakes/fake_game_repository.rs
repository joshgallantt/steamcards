use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;

use crate::{Game, GameRepository, SteamLibrary};

/// A library held in memory.
#[derive(Default)]
pub struct FakeGameRepository {
    pub library: Mutex<SteamLibrary>,
    /// Everything fails, as if Steam were down.
    pub down: AtomicBool,
}

impl FakeGameRepository {
    pub fn with(games: Vec<Game>) -> Self {
        Self {
            library: Mutex::new(SteamLibrary::new(games)),
            down: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl GameRepository for FakeGameRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("steamcommunity.com didn't answer");
        }
        Ok(self.library.lock().unwrap().clone())
    }
}

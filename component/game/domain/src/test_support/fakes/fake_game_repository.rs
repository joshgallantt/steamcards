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
    /// Another session signed on in this one's place: reads fail for that.
    pub replaced: AtomicBool,
}

impl FakeGameRepository {
    pub fn with(games: Vec<Game>) -> Self {
        Self {
            library: Mutex::new(SteamLibrary::new(games)),
            down: AtomicBool::new(false),
            replaced: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl GameRepository for FakeGameRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        if self.replaced() {
            anyhow::bail!("the connection to Steam closed");
        }
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("steamcommunity.com didn't answer");
        }
        Ok(self.library.lock().unwrap().clone())
    }

    fn replaced(&self) -> bool {
        self.replaced.load(Ordering::Relaxed)
    }
}

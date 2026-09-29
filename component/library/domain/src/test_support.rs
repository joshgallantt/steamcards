//! Doubles for other crates' tests. A double stands in for the contract, so
//! no test needs Steam.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;

use crate::{
    CardDrops, Game, LibraryError, LibraryRepository, LookAtGame, ReadLibrary, SteamLibrary,
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

/// A library held in memory.
#[derive(Default)]
pub struct InMemoryLibraryRepository {
    pub library: Mutex<SteamLibrary>,
    /// Reads fail, as if steamcommunity.com were down.
    pub down: AtomicBool,
}

impl InMemoryLibraryRepository {
    pub fn with(games: Vec<Game>) -> Self {
        Self {
            library: Mutex::new(SteamLibrary::new(games)),
            down: AtomicBool::new(false),
        }
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
}

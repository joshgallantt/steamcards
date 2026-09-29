//! The library domain's contract, satisfied by steamcommunity.com: badge
//! pages and card pages in, the library's entities out. Imports `library`
//! because the contract is declared there; `library` imports nothing back.

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use library::{Card, CardDrops, Game, LibraryRepository, SteamLibrary};
use steam_api::{Session, badges::BadgeGame};

/// The account's badges, read signed in.
pub struct SteamLibraryRepository {
    session: Arc<Session>,
}

impl SteamLibraryRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl LibraryRepository for SteamLibraryRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        let games = self.session.badges().await?;
        Ok(SteamLibrary::new(games.into_iter().map(to_game).collect()))
    }

    async fn game(&self, app_id: u32) -> anyhow::Result<Game> {
        self.session
            .game_cards(app_id)
            .await?
            .map(to_game)
            .ok_or_else(|| anyhow!("its card page has no card drops to read"))
    }
}

fn to_game(b: BadgeGame) -> Game {
    Game {
        app_id: b.app_id,
        name: b.name,
        hours: b.hours,
        drops: CardDrops {
            received: b.cards_received,
            remaining: b.cards_left,
        },
        badge_level: b.badge_level,
        cards: b
            .cards
            .into_iter()
            .map(|c| Card {
                name: c.name,
                owned: c.owned,
            })
            .collect(),
    }
}

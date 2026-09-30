//! The game domain's contract, satisfied by Steam: the badge pages from
//! steamcommunity.com in, the library out. Imports `game` because the
//! contract is declared there; `game` imports nothing back.

use std::sync::Arc;

use async_trait::async_trait;
use game::{CardDrops, Game, GameRepository, SteamLibrary};
use steam_api::{Session, badges::BadgeGame};

/// The account's badges, read signed in.
pub struct SteamGameRepository {
    session: Arc<Session>,
}

impl SteamGameRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl GameRepository for SteamGameRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        let games = self.session.badges().await?;
        Ok(SteamLibrary::new(games.into_iter().map(to_game).collect()))
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
    }
}

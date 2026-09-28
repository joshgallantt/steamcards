use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use farming::{CardsRepository, Game};
use steam_api::{Session, badges::BadgeGame};

/// The badge pages on steamcommunity.com, read signed in.
pub struct SteamCardsRepository {
    session: Arc<Session>,
}

impl SteamCardsRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl CardsRepository for SteamCardsRepository {
    async fn games(&self) -> anyhow::Result<Vec<Game>> {
        Ok(self
            .session
            .badges()
            .await?
            .into_iter()
            .map(to_game)
            .collect())
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
        cards_left: b.cards_left,
        cards_dropped: b.cards_received,
    }
}

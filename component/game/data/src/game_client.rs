use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use game::{AppId, CardDrops, Game};
use steam_api::{SteamClient, badges::BadgeGame};

use crate::badge_page::read_badge_page;

/// Badge pages read at most: 150 games each, so a very large library.
const MAX_PAGES: u32 = 100;

/// Steam's say on the account's games.
#[async_trait]
pub trait GameClient: Send + Sync {
    /// Every game with trading cards on the account.
    async fn games(&self) -> anyhow::Result<Vec<Game>>;
}

/// The account's badge pages, read signed in, as the Steam client fetches
/// them.
pub struct SteamGameClient {
    steam: Arc<SteamClient>,
}

impl SteamGameClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self { steam }
    }
}

#[async_trait]
impl GameClient for SteamGameClient {
    /// Every page of badges, in order. Games the badge pages may be wrong
    /// about are checked on their own card page.
    async fn games(&self) -> anyhow::Result<Vec<Game>> {
        let path = |id: u64, page: u32| format!("/profiles/{id}/badges?l=english&p={page}");
        let (first, who) = self.steam.page_as_owner(|id| path(id, 1)).await?;
        let first = read_badge_page(&first);
        let mut badges = first.games;
        for page in 2..=first.pages.min(MAX_PAGES) {
            let html = self.steam.page(&path(who.steam_id(), page), &who).await?;
            badges.extend(read_badge_page(&html).games);
        }
        // A game can move to the next page while they're read.
        let mut seen = HashSet::new();
        badges.retain(|b| seen.insert(b.app_id));
        let mut games = Vec::with_capacity(badges.len());
        for badge in badges {
            if !badge.unsure {
                games.push(badge.into_domain());
                continue;
            }
            match self.steam.game_cards(badge.app_id).await {
                Ok(Some(checked)) => games.push(card_page_game(checked)),
                Ok(None) => games.push(badge.into_domain()),
                Err(e) => {
                    self.steam
                        .log()
                        .line(&format!("checking {}: {e}", badge.app_id));
                    games.push(badge.into_domain());
                }
            }
        }
        Ok(games)
    }
}

/// A game as its own card page shows it.
fn card_page_game(b: BadgeGame) -> Game {
    Game {
        app_id: AppId(b.app_id),
        name: b.name,
        hours: b.hours,
        drops: CardDrops {
            received: b.cards_received,
            remaining: b.cards_left,
        },
        badge_level: b.badge_level,
    }
}

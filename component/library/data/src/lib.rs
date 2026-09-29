//! The library domain's contract, satisfied by Steam: badge pages and card
//! pages from steamcommunity.com, and the inventory's items over the CM
//! connection, in; the library's entities out. Imports `library` because
//! the contract is declared there; `library` imports nothing back.

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use library::{Card, CardAsset, CardDrops, Game, LibraryRepository, SteamLibrary};
use steam_api::{Session, badges::BadgeGame, inventory::InventoryItem};

/// The account's badges, read signed in, and the items it holds.
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

    async fn describe(&self, asset_ids: &[u64]) -> anyhow::Result<Vec<CardAsset>> {
        let items = self.session.describe_items(asset_ids).await?;
        Ok(items.into_iter().filter_map(to_card_asset).collect())
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

/// The copy of a card an item is; `None` when it isn't a trading card, or
/// Steam doesn't say which game's it is.
fn to_card_asset(item: InventoryItem) -> Option<CardAsset> {
    if !item.trading_card {
        return None;
    }
    Some(CardAsset {
        asset_id: item.asset_id,
        app_id: item.app_id?,
        name: item.name,
        market_hash_name: item.market_hash_name,
        foil: item.foil,
        marketable: item.marketable,
        tradable: item.tradable,
    })
}

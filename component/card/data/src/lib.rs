//! The card domain's contract, satisfied by Steam: card pages (foils' too)
//! from steamcommunity.com, and the inventory's items over the CM
//! connection, in; the card domain's entities out. Imports `card` because
//! the contract is declared there; `card` imports nothing back.

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use card::{Card, CardAsset, CardRepository, CardSet, GameCards};
use game::{CardDrops, Game};
use steam_api::{
    SteamClient,
    badges::{BadgeGame, SetCard},
    inventory::InventoryItem,
};

/// The account's card pages, read signed in, and the items it holds.
pub struct SteamCardRepository {
    steam: Arc<SteamClient>,
}

impl SteamCardRepository {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self { steam }
    }
}

#[async_trait]
impl CardRepository for SteamCardRepository {
    async fn game_cards(&self, app_id: u32) -> anyhow::Result<GameCards> {
        self.steam
            .game_cards(app_id)
            .await?
            .map(to_game_cards)
            .ok_or_else(|| anyhow!("its card page has no card drops to read"))
    }

    async fn foils(&self, app_id: u32) -> anyhow::Result<CardSet> {
        let foils = self.steam.foil_cards(app_id).await?;
        Ok(CardSet::new(foils.into_iter().map(to_card).collect()))
    }

    async fn describe(&self, asset_ids: &[u64]) -> anyhow::Result<Vec<CardAsset>> {
        let items = self.steam.describe_items(asset_ids).await?;
        Ok(items.into_iter().filter_map(to_card_asset).collect())
    }
}

fn to_game_cards(b: BadgeGame) -> GameCards {
    GameCards {
        game: Game {
            app_id: b.app_id,
            name: b.name,
            hours: b.hours,
            drops: CardDrops {
                received: b.cards_received,
                remaining: b.cards_left,
            },
            badge_level: b.badge_level,
        },
        set: CardSet::new(b.cards.into_iter().map(to_card).collect()),
    }
}

fn to_card(c: SetCard) -> Card {
    Card {
        name: c.name,
        owned: c.owned,
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

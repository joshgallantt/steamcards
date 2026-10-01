use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use card::{AssetId, Card, CardAsset, CardSet, GameCards};
use steam_api::{
    SteamClient,
    badges::{BadgeGame, SetCard},
    inventory::InventoryItem,
};
use steam_library::{AppId, CardDrops, Game};

/// Steam's say on the account's cards.
#[async_trait]
pub trait CardClient: Send + Sync {
    /// A game's card page, looked at afresh: the game's drops and hours, and
    /// its set.
    async fn game_cards(&self, app_id: AppId) -> anyhow::Result<GameCards>;

    /// A game's foils, from its foil badge's card page.
    async fn foils(&self, app_id: AppId) -> anyhow::Result<CardSet>;

    /// Which cards these items are: each copy on its own, cards only.
    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>>;
}

/// The account's card pages, read signed in, and the items it holds, as the
/// Steam client reads them.
pub struct SteamCardClient {
    steam: Arc<SteamClient>,
}

impl SteamCardClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self { steam }
    }
}

#[async_trait]
impl CardClient for SteamCardClient {
    async fn game_cards(&self, app_id: AppId) -> anyhow::Result<GameCards> {
        self.steam
            .game_cards(app_id.0)
            .await?
            .map(to_game_cards)
            .ok_or_else(|| anyhow!("its card page has no card drops to read"))
    }

    async fn foils(&self, app_id: AppId) -> anyhow::Result<CardSet> {
        let foils = self.steam.foil_cards(app_id.0).await?;
        Ok(CardSet::new(foils.into_iter().map(to_card).collect()))
    }

    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>> {
        let ids: Vec<u64> = asset_ids.iter().map(|id| id.0).collect();
        let items = self.steam.describe_items(&ids).await?;
        Ok(items.into_iter().filter_map(to_card_asset).collect())
    }
}

fn to_game_cards(b: BadgeGame) -> GameCards {
    GameCards {
        game: Game {
            app_id: AppId(b.app_id),
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
        asset_id: AssetId(item.asset_id),
        app_id: AppId(item.app_id?),
        name: item.name,
        market_hash_name: item.market_hash_name,
        foil: item.foil,
        marketable: item.marketable,
        tradable: item.tradable,
    })
}

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use async_trait::async_trait;
use card::{AssetId, Card, CardAsset, CardKind, CardSet, GameCards, NewItem};
use chrono::DateTime;
use game::{AppId, CardDrops, Game};
use steam_api::{
    SteamClient,
    badges::{BadgeGame, SetCard},
    cm::{Announced, Announcement, UnseenItem},
    inventory::InventoryItem,
};
use tokio::sync::broadcast;

/// Steam's say on the account's cards.
#[async_trait]
pub trait CardClient: Send + Sync {
    /// A game's card page, looked at afresh: the game's drops and hours, and
    /// its set.
    async fn game_cards(&self, app_id: AppId) -> anyhow::Result<GameCards>;

    /// A game's set in foil, from its foil badge's card page.
    async fn foils(&self, app_id: AppId) -> anyhow::Result<CardSet>;

    /// Which cards these items are: each copy on its own, cards only.
    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>>;

    /// What Steam says next is new: the community items it lists that
    /// weren't heard of before, or none when it only counts more.
    async fn next_new_items(&self) -> Vec<NewItem>;
}

/// The account's card pages, read signed in, the items it holds, and what
/// Steam announces is new, as the Steam client reads them.
pub struct SteamCardClient {
    steam: Arc<SteamClient>,
    /// What Steam announces on every connection signed on, from when this
    /// client was made.
    announced: tokio::sync::Mutex<broadcast::Receiver<Announced>>,
    heard: Mutex<Heard>,
}

/// What Steam has said is new. It says so again with every announcement
/// until the inventory is viewed, so each item is passed on once.
#[derive(Default)]
struct Heard {
    items: HashSet<u64>,
    /// How many new items Steam last counted.
    count: u32,
    /// The account whose first sign-on here was taken as what was new
    /// already. Signing on again, it's what's new since.
    baseline: Option<u64>,
}

impl Heard {
    /// Takes in an announcement for `account`: the community items it lists
    /// that weren't heard of before. When it lists none such but counts more
    /// than before, none: a card may have dropped all the same. `None` when
    /// nothing is new, or when it's what was there already when the account
    /// first signed on here.
    fn hear(&mut self, a: &Announcement, account: u64) -> Option<Vec<NewItem>> {
        let fresh: Vec<NewItem> = a
            .items
            .iter()
            .filter(|i| i.is_community_item() && self.items.insert(i.asset_id))
            .map(new_item)
            .collect();
        let more = a.count > self.count;
        self.count = a.count;
        if a.at_sign_on && self.baseline != Some(account) {
            self.baseline = Some(account);
            return None;
        }
        (!fresh.is_empty() || more).then_some(fresh)
    }
}

impl SteamCardClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self {
            announced: tokio::sync::Mutex::new(steam.announcements()),
            steam,
            heard: Mutex::default(),
        }
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
        Ok(CardSet::new(
            CardKind::Foil,
            foils.into_iter().map(to_card).collect(),
        ))
    }

    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>> {
        let ids: Vec<u64> = asset_ids.iter().map(|id| id.0).collect();
        let items = self.steam.describe_items(&ids).await?;
        Ok(items.into_iter().filter_map(to_card_asset).collect())
    }

    async fn next_new_items(&self) -> Vec<NewItem> {
        let mut announced = self.announced.lock().await;
        loop {
            match announced.recv().await {
                Ok(said) => {
                    let heard = self
                        .heard
                        .lock()
                        .unwrap()
                        .hear(&said.announcement, said.account);
                    if let Some(items) = heard {
                        return items;
                    }
                }
                // Missed some: the next lists every item not seen yet again.
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                // The Steam client is gone: nothing more will be said.
                Err(broadcast::error::RecvError::Closed) => {
                    return std::future::pending().await;
                }
            }
        }
    }
}

/// A game and its set, as its card page shows them. The page doesn't say
/// whether the game is private, or when it was bought: the library does.
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
            private: false,
            bought_at: None,
        },
        set: CardSet::new(CardKind::Normal, b.cards.into_iter().map(to_card).collect()),
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
        // Steam tags a copy with its border: cardborder_1 is a foil.
        kind: if item.foil {
            CardKind::Foil
        } else {
            CardKind::Normal
        },
        marketable: item.marketable,
        tradable: item.tradable,
    })
}

/// An item Steam listed, as the card component knows it: the game it came
/// from is its source app.
fn new_item(item: &UnseenItem) -> NewItem {
    NewItem {
        asset_id: AssetId(item.asset_id),
        app_id: item.source_app_id.map(AppId),
        gained_at: item
            .gained_at
            .and_then(|at| DateTime::from_timestamp(i64::from(at), 0)),
    }
}

use async_trait::async_trait;

use game::AppId;

use crate::{AssetId, CardAsset, CardSet, GameCards, NewItem};

/// Where the cards come from. Declared here, beside the use cases that need
/// it; the data layer is written to fit.
///
/// Errors carry a reason fit to show the user.
#[async_trait]
pub trait CardRepository: Send + Sync {
    /// One game looked at afresh on its own card page: its drops and hours,
    /// and its card set.
    async fn game_cards(&self, app_id: AppId) -> anyhow::Result<GameCards>;

    /// One game's set in foil looked at afresh: each card of it, and how
    /// many foils of it the account has. Its card page counts the normal
    /// set only; foils make a badge of their own, on a page of their own.
    async fn foils(&self, app_id: AppId) -> anyhow::Result<CardSet>;

    /// The trading cards among the account's items with these asset IDs,
    /// each once, in the order asked. Other items (emoticons, backgrounds,
    /// gems, booster packs) are left out, and so are IDs Steam doesn't know.
    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>>;

    /// What Steam says next is new in the account's inventory, once it says
    /// it: the community items it lists that weren't heard of before, or
    /// none when it only counts more than before. A card may have dropped
    /// either way. What was new before steamcards first signed on isn't
    /// news. Dropped before Steam has said, nothing is missed: the next call
    /// hears it.
    async fn next_new_items(&self) -> Vec<NewItem>;
}

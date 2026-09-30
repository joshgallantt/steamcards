use async_trait::async_trait;

use crate::{Card, CardAsset, Game, SteamLibrary};

/// Where the library comes from. Declared here, beside the use cases that
/// need it; the data layer is written to fit.
///
/// Errors carry a reason fit to show the user.
#[async_trait]
pub trait LibraryRepository: Send + Sync {
    /// Every game on the account that has trading cards, finished ones
    /// included. Card sets aren't read here: that's a page per game.
    async fn library(&self) -> anyhow::Result<SteamLibrary>;

    /// One game looked at afresh: its drops, hours and card set.
    async fn game(&self, app_id: u32) -> anyhow::Result<Game>;

    /// One game's foils looked at afresh: each foil card of its set, and how
    /// many the account has. A game's set counts its normal cards only;
    /// foils make a badge of their own, on a page of their own.
    async fn foils(&self, app_id: u32) -> anyhow::Result<Vec<Card>>;

    /// The trading cards among the account's items with these asset IDs,
    /// each once, in the order asked. Other items (emoticons, backgrounds,
    /// gems, booster packs) are left out, and so are IDs Steam doesn't know.
    async fn describe(&self, asset_ids: &[u64]) -> anyhow::Result<Vec<CardAsset>>;
}

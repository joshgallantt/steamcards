use async_trait::async_trait;

use crate::{Game, SteamLibrary};

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
}

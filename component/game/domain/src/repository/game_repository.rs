use async_trait::async_trait;

use crate::SteamLibrary;

/// Where the library comes from. Declared here, beside the use case that
/// needs it; the data layer is written to fit.
///
/// Errors carry a reason fit to show the user.
#[async_trait]
pub trait GameRepository: Send + Sync {
    /// Every game on the account that has trading cards, finished ones
    /// included. Card sets aren't read here: that's a page per game.
    async fn library(&self) -> anyhow::Result<SteamLibrary>;

    /// Whether Steam signed the last session off for another one signed on
    /// in its place, with this sign-in, until a session signs on here
    /// again: whatever was asked of Steam meanwhile failed for that.
    fn replaced(&self) -> bool;
}

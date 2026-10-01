use async_trait::async_trait;

use crate::SteamLibrary;

/// Where the library comes from. Declared here, beside the use cases that
/// need it; the data layer is written to fit.
///
/// Errors carry a reason fit to show the user.
#[async_trait]
pub trait SteamLibraryRepository: Send + Sync {
    /// Every game on the account that has trading cards, finished ones
    /// included. Card sets aren't read here: that's a page per game.
    async fn library(&self) -> anyhow::Result<SteamLibrary>;
}

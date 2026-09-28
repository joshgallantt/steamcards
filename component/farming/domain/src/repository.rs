use async_trait::async_trait;

use crate::{Game, Signal};

/// Which games have cards, and how many are left: the badges. Declared here,
/// beside the use cases that need it; the data layer is written to fit.
///
/// Errors carry a reason fit to show the user.
#[async_trait]
pub trait CardsRepository: Send + Sync {
    /// Every game with trading cards the account has, finished ones included.
    async fn games(&self) -> anyhow::Result<Vec<Game>>;

    /// One game's cards, looked at afresh.
    async fn game(&self, app_id: u32) -> anyhow::Result<Game>;
}

/// Playing games on Steam. Declared here, beside the use case that needs it.
#[async_trait]
pub trait PlayRepository: Send + Sync {
    /// Plays exactly these games, signing on first if need be, and shows to
    /// friends as online or not.
    async fn play(&self, app_ids: &[u32], online: bool) -> anyhow::Result<()>;

    /// Stops playing, and signs off.
    async fn stop(&self);

    /// Whether another device's game blocks playing right now, as Steam last
    /// said, and what it's playing when Steam says.
    fn blocked(&self) -> Option<Option<u32>>;

    /// Waits for Steam to say something the farmer acts on.
    async fn next_signal(&self) -> Signal;
}

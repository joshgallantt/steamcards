use async_trait::async_trait;

use steam_library::AppId;

use crate::Signal;

/// Playing games on Steam. Declared here, beside the use case that needs it;
/// the data layer is written to fit.
#[async_trait]
pub trait FarmingRepository: Send + Sync {
    /// Plays exactly these games, signing on first if need be, and shows to
    /// friends as online or not. While another device plays, it plays
    /// nothing: Steam would sign this session off. Errs with a reason fit to
    /// show the user.
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()>;

    /// Signs on if need be, playing nothing, so Steam can say when another
    /// device stops playing. Errs with a reason fit to show the user.
    async fn listen(&self) -> anyhow::Result<()>;

    /// Stops playing, and signs off.
    async fn stop(&self);

    /// Whether another device's game blocks playing right now, as Steam last
    /// said, and what it's playing when Steam says. Signed off, nothing is
    /// said.
    fn blocked(&self) -> Option<Option<AppId>>;

    /// Waits for Steam to say something the farmer acts on.
    async fn next_signal(&self) -> Signal;
}

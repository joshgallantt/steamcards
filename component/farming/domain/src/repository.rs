use async_trait::async_trait;

use crate::Signal;

/// Playing games on Steam. Declared here, beside the use case that needs it;
/// the data layer is written to fit.
#[async_trait]
pub trait PlayRepository: Send + Sync {
    /// Plays exactly these games, signing on first if need be, and shows to
    /// friends as online or not. Errs with a reason fit to show the user.
    async fn play(&self, app_ids: &[u32], online: bool) -> anyhow::Result<()>;

    /// Stops playing, and signs off.
    async fn stop(&self);

    /// Whether another device's game blocks playing right now, as Steam last
    /// said, and what it's playing when Steam says.
    fn blocked(&self) -> Option<Option<u32>>;

    /// Waits for Steam to say something the farmer acts on.
    async fn next_signal(&self) -> Signal;
}

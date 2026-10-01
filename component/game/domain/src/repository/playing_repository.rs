use async_trait::async_trait;

use crate::{AppId, Playing, PlayingSignal};

/// Playing games on Steam. Declared here, beside the use cases that need
/// it; the data layer is written to fit.
#[async_trait]
pub trait PlayingRepository: Send + Sync {
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

    /// Who plays on the account right now, as Steam last said: another
    /// device, and its game when Steam says, or this session. Signed off,
    /// nothing is said: this session, then.
    fn playing(&self) -> Playing;

    /// Waits for Steam to say something about playing. Dropped before it
    /// has, nothing is missed: the next call hears it.
    async fn next_signal(&self) -> PlayingSignal;
}

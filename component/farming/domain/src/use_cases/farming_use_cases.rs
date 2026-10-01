//! Everything asked of farming, a trait each. Each is done by a
//! `Default…UseCase` in `impl/`, over the farmer.

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::FarmingUpdate;

/// Farms until the token is cancelled, telling the channel what happens
/// and where farming stands. Runs detached; the handle resolves once it has
/// stopped, and stopped playing. A run carries on the session the last one
/// left, until it's ended.
pub trait FarmCardsUseCase: Send + Sync {
    fn call(
        &self,
        token: CancellationToken,
        updates: mpsc::Sender<FarmingUpdate>,
    ) -> JoinHandle<()>;
}

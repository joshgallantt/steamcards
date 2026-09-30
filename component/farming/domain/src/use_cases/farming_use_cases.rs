//! Everything asked of farming, a trait each. Each is done by a
//! `Default…UseCase` in `impl/`, over the farmer.

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::FarmingEvent;

/// Farms until the token is cancelled, reporting on the channel. Runs
/// detached; the handle resolves once it has stopped, and stopped playing.
/// A run carries on the session the last one left, until it's ended.
pub trait FarmCardsUseCase: Send + Sync {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<FarmingEvent>) -> JoinHandle<()>;
}

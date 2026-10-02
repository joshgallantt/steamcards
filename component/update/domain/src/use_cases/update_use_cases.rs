//! Everything asked of steamcards' updates, a trait each. Each is done over
//! the repository by a `Default…UseCase` in `impl/`.

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::UpdateEvent;

/// Keeps steamcards up to date in the background, until the token is
/// cancelled: looks for a newer release now, and once a day after, while
/// automatic updates are on. A release this copy updates itself to is put
/// in place, for the next start; one Homebrew or cargo looks after is said
/// to be out. Each release is told of on the channel once.
pub trait KeepUpToDateUseCase: Send + Sync {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<UpdateEvent>) -> JoinHandle<()>;
}

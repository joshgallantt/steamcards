use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{KeepUpToDateUseCase, UpdateEvent};

/// Keeps nothing up to date: says the events it's given, then waits to be
/// cancelled, counting how often it was started.
#[derive(Default)]
pub struct SpyKeepUpToDateUseCase {
    pub starts: AtomicUsize,
    /// What it says once started.
    pub says: Vec<UpdateEvent>,
}

impl KeepUpToDateUseCase for SpyKeepUpToDateUseCase {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<UpdateEvent>) -> JoinHandle<()> {
        self.starts.fetch_add(1, Ordering::Relaxed);
        let says = self.says.clone();
        tokio::spawn(async move {
            for event in says {
                if events.send(event).await.is_err() {
                    return;
                }
            }
            token.cancelled().await;
        })
    }
}

use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{KeepPricesUpToDateUseCase, PriceEvent};

/// Prices nothing, and stops at once, counting how often it was started.
#[derive(Default)]
pub struct SpyKeepPricesUpToDateUseCase {
    starts: AtomicUsize,
}

impl SpyKeepPricesUpToDateUseCase {
    pub fn starts(&self) -> usize {
        self.starts.load(Ordering::Relaxed)
    }
}

impl KeepPricesUpToDateUseCase for SpyKeepPricesUpToDateUseCase {
    fn call(&self, _: CancellationToken, _: mpsc::Sender<PriceEvent>) -> JoinHandle<()> {
        self.starts.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async {})
    }
}

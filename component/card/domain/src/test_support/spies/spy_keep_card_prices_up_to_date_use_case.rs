use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{KeepCardPricesUpToDateUseCase, PriceEvent};

/// Prices nothing, and stops at once, counting how often it was started.
#[derive(Default)]
pub struct SpyKeepCardPricesUpToDateUseCase {
    starts: AtomicUsize,
}

impl SpyKeepCardPricesUpToDateUseCase {
    pub fn starts(&self) -> usize {
        self.starts.load(Ordering::Relaxed)
    }
}

impl KeepCardPricesUpToDateUseCase for SpyKeepCardPricesUpToDateUseCase {
    fn call(&self, _: CancellationToken, _: mpsc::Sender<PriceEvent>) -> JoinHandle<()> {
        self.starts.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async {})
    }
}

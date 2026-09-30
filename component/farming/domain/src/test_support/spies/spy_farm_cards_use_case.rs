use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{FarmCardsUseCase, FarmingEvent};

/// Never farms: each run stops at once. Counts the runs started, for screens
/// that need a farmer to exist.
#[derive(Default)]
pub struct SpyFarmCardsUseCase {
    runs: AtomicUsize,
}

impl SpyFarmCardsUseCase {
    pub fn runs(&self) -> usize {
        self.runs.load(Ordering::Relaxed)
    }
}

impl FarmCardsUseCase for SpyFarmCardsUseCase {
    fn call(&self, _: CancellationToken, _: mpsc::Sender<FarmingEvent>) -> JoinHandle<()> {
        self.runs.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async {})
    }
}

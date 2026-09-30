use std::sync::Arc;

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{Clock, KeepPricesUpToDateUseCase, PriceEvent, PriceRepository, watcher::Watcher};

pub struct DefaultKeepPricesUpToDateUseCase {
    repo: Arc<dyn PriceRepository>,
    clock: Clock,
}

impl DefaultKeepPricesUpToDateUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>, clock: Clock) -> Self {
        Self { repo, clock }
    }
}

impl KeepPricesUpToDateUseCase for DefaultKeepPricesUpToDateUseCase {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<PriceEvent>) -> JoinHandle<()> {
        let watcher = Watcher::new(Arc::clone(&self.repo), Arc::clone(&self.clock), events);
        tokio::spawn(async move { watcher.run(&token).await })
    }
}

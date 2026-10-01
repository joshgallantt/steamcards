use std::sync::Arc;

use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use super::price_watcher::PriceWatcher;
use crate::{CardPriceRepository, Clock, KeepCardPricesUpToDateUseCase, PriceEvent};

pub struct DefaultKeepCardPricesUpToDateUseCase {
    repo: Arc<dyn CardPriceRepository>,
    clock: Clock,
}

impl DefaultKeepCardPricesUpToDateUseCase {
    pub fn new(repo: Arc<dyn CardPriceRepository>, clock: Clock) -> Self {
        Self { repo, clock }
    }
}

impl KeepCardPricesUpToDateUseCase for DefaultKeepCardPricesUpToDateUseCase {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<PriceEvent>) -> JoinHandle<()> {
        let watcher = PriceWatcher::new(Arc::clone(&self.repo), Arc::clone(&self.clock), events);
        tokio::spawn(async move { watcher.run(&token).await })
    }
}

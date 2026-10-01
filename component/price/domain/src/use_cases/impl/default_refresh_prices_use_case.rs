use std::sync::Arc;

use game::AppId;
use tokio::task::JoinHandle;

use crate::{
    Clock, PriceError, PriceRepository, RefreshPricesUseCase,
    pricing::{Priced, price_set},
    rules::{ASKED_AGAIN_AFTER, between},
};

pub struct DefaultRefreshPricesUseCase {
    repo: Arc<dyn PriceRepository>,
    clock: Clock,
}

impl DefaultRefreshPricesUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>, clock: Clock) -> Self {
        Self { repo, clock }
    }
}

impl RefreshPricesUseCase for DefaultRefreshPricesUseCase {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<(), PriceError>> {
        let (repo, clock) = (Arc::clone(&self.repo), Arc::clone(&self.clock));
        tokio::spawn(async move {
            let recent = repo
                .book()
                .sets
                .get(&app_id)
                .is_some_and(|set| between(set.fetched_at, clock()) <= ASKED_AGAIN_AFTER);
            if recent {
                return Ok(());
            }
            match price_set(&*repo, app_id, &clock).await {
                Priced::Paused(pause) => Err(PriceError::Paused(pause)),
                Priced::Unanswered(_) => Err(PriceError::Unanswered),
                Priced::Done | Priced::Failed(_) => Ok(()),
            }
        })
    }
}

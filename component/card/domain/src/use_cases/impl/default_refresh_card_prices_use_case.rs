use std::sync::Arc;

use game::AppId;
use tokio::task::JoinHandle;

use crate::{
    CardPriceRepository, Clock, PriceError, RefreshCardPricesUseCase,
    model::rules::{ASKED_AGAIN_AFTER, between},
};

use super::pricing::{Priced, price_set};

pub struct DefaultRefreshCardPricesUseCase {
    repo: Arc<dyn CardPriceRepository>,
    clock: Clock,
}

impl DefaultRefreshCardPricesUseCase {
    pub fn new(repo: Arc<dyn CardPriceRepository>, clock: Clock) -> Self {
        Self { repo, clock }
    }
}

impl RefreshCardPricesUseCase for DefaultRefreshCardPricesUseCase {
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

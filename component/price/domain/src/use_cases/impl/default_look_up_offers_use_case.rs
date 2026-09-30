use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, Utc};
use tokio::task::JoinHandle;

use crate::{
    Clock, LookUpOffersUseCase, Lookup, Offers, Price, PriceError, PriceRepository,
    rules::{OFFERS_FRESH_FOR, between},
};

pub struct DefaultLookUpOffersUseCase {
    repo: Arc<dyn PriceRepository>,
    clock: Clock,
}

impl DefaultLookUpOffersUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>, clock: Clock) -> Self {
        Self { repo, clock }
    }
}

impl LookUpOffersUseCase for DefaultLookUpOffersUseCase {
    fn call(&self, market_hash_names: Vec<String>) -> JoinHandle<Result<(), PriceError>> {
        let (repo, clock) = (Arc::clone(&self.repo), Arc::clone(&self.clock));
        tokio::spawn(async move {
            let mut seen = HashSet::new();
            for hash in market_hash_names
                .into_iter()
                .filter(|h| seen.insert(h.clone()))
            {
                let book = repo.book();
                if is_recent(book.offers.get(&hash), clock()) {
                    continue;
                }
                let price = match repo.look_up_offers(&hash).await {
                    Ok(Lookup::Found(price)) => price,
                    Ok(Lookup::Paused(pause)) => return Err(PriceError::Paused(pause)),
                    Ok(Lookup::Unanswered(_)) => return Err(PriceError::Unanswered),
                    Err(_) => Price::failed(clock()),
                };
                let looked_up_at = clock();
                repo.keep_offers(
                    &hash,
                    Offers {
                        price,
                        looked_up_at,
                    },
                );
            }
            Ok(())
        })
    }
}

/// An order book looked up under half an hour ago, or a failed one not due
/// again yet.
fn is_recent(offers: Option<&Offers>, now: DateTime<Utc>) -> bool {
    match offers {
        Some(Offers {
            price: Price::Failed { retry_at },
            ..
        }) => *retry_at > now,
        Some(offers) => between(offers.looked_up_at, now) < OFFERS_FRESH_FOR,
        None => false,
    }
}

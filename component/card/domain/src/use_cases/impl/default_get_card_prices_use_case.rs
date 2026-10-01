use std::sync::Arc;

use crate::{CardPriceRepository, GetCardPricesUseCase, PriceBook};

pub struct DefaultGetCardPricesUseCase {
    repo: Arc<dyn CardPriceRepository>,
}

impl DefaultGetCardPricesUseCase {
    pub fn new(repo: Arc<dyn CardPriceRepository>) -> Self {
        Self { repo }
    }
}

impl GetCardPricesUseCase for DefaultGetCardPricesUseCase {
    fn call(&self) -> Arc<PriceBook> {
        self.repo.book()
    }
}

use std::sync::Arc;

use crate::{GetPricesUseCase, PriceBook, PriceRepository};

pub struct DefaultGetPricesUseCase {
    repo: Arc<dyn PriceRepository>,
}

impl DefaultGetPricesUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>) -> Self {
        Self { repo }
    }
}

impl GetPricesUseCase for DefaultGetPricesUseCase {
    fn call(&self) -> Arc<PriceBook> {
        self.repo.book()
    }
}

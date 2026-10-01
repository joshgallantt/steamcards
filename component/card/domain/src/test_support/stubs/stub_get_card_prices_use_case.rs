use std::sync::Arc;

use crate::{GetCardPricesUseCase, PriceBook};

/// Answers every read with the same book; by default an empty one, with
/// every price on its way.
#[derive(Default)]
pub struct StubGetCardPricesUseCase {
    book: Arc<PriceBook>,
}

impl StubGetCardPricesUseCase {
    pub fn new(book: PriceBook) -> Self {
        Self {
            book: Arc::new(book),
        }
    }
}

impl GetCardPricesUseCase for StubGetCardPricesUseCase {
    fn call(&self) -> Arc<PriceBook> {
        Arc::clone(&self.book)
    }
}

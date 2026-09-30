use std::sync::Arc;

use crate::{GetPricesUseCase, PriceBook};

/// Answers every read with the same book; by default an empty one, with
/// every price on its way.
#[derive(Default)]
pub struct StubGetPricesUseCase {
    book: Arc<PriceBook>,
}

impl StubGetPricesUseCase {
    pub fn new(book: PriceBook) -> Self {
        Self {
            book: Arc::new(book),
        }
    }
}

impl GetPricesUseCase for StubGetPricesUseCase {
    fn call(&self) -> Arc<PriceBook> {
        Arc::clone(&self.book)
    }
}

use std::sync::Arc;

use async_trait::async_trait;

use crate::{CardRepository, NewItem, ObserveNewItemsUseCase};

pub struct DefaultObserveNewItemsUseCase {
    repo: Arc<dyn CardRepository>,
}

impl DefaultObserveNewItemsUseCase {
    pub fn new(repo: Arc<dyn CardRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ObserveNewItemsUseCase for DefaultObserveNewItemsUseCase {
    async fn call(&self) -> Vec<NewItem> {
        self.repo.next_new_items().await
    }
}

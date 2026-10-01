use std::sync::Arc;

use async_trait::async_trait;

use crate::{ObservePlayingUseCase, PlayingRepository, PlayingSignal};

pub struct DefaultObservePlayingUseCase {
    repo: Arc<dyn PlayingRepository>,
}

impl DefaultObservePlayingUseCase {
    pub fn new(repo: Arc<dyn PlayingRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ObservePlayingUseCase for DefaultObservePlayingUseCase {
    async fn call(&self) -> PlayingSignal {
        self.repo.next_signal().await
    }
}

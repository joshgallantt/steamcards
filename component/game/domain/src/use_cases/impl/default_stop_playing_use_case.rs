use std::sync::Arc;

use async_trait::async_trait;

use crate::{PlayingRepository, StopPlayingUseCase};

pub struct DefaultStopPlayingUseCase {
    repo: Arc<dyn PlayingRepository>,
}

impl DefaultStopPlayingUseCase {
    pub fn new(repo: Arc<dyn PlayingRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl StopPlayingUseCase for DefaultStopPlayingUseCase {
    async fn call(&self) {
        self.repo.stop().await;
    }
}

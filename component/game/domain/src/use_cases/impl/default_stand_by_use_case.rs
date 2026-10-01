use std::sync::Arc;

use async_trait::async_trait;

use crate::{GameError, Playing, PlayingRepository, StandByUseCase};

pub struct DefaultStandByUseCase {
    repo: Arc<dyn PlayingRepository>,
}

impl DefaultStandByUseCase {
    pub fn new(repo: Arc<dyn PlayingRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl StandByUseCase for DefaultStandByUseCase {
    async fn call(&self) -> Result<Playing, GameError> {
        self.repo
            .listen()
            .await
            .map_err(|e| GameError::Unavailable(e.to_string()))?;
        Ok(self.repo.playing())
    }
}

use std::sync::Arc;

use async_trait::async_trait;

use crate::{AppId, GameError, PlayGamesUseCase, Playing, PlayingRepository};

pub struct DefaultPlayGamesUseCase {
    repo: Arc<dyn PlayingRepository>,
}

impl DefaultPlayGamesUseCase {
    pub fn new(repo: Arc<dyn PlayingRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl PlayGamesUseCase for DefaultPlayGamesUseCase {
    async fn call(&self, app_ids: &[AppId], online: bool) -> Result<Playing, GameError> {
        self.repo
            .play(app_ids, online)
            .await
            .map_err(|e| GameError::Unavailable(e.to_string()))?;
        Ok(self.repo.playing())
    }
}

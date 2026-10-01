use std::sync::Arc;

use steam_library::AppId;
use tokio::task::JoinHandle;

use crate::{CardError, CardRepository, GameCards, LookAtCardsUseCase};

pub struct DefaultLookAtCardsUseCase {
    repo: Arc<dyn CardRepository>,
}

impl DefaultLookAtCardsUseCase {
    pub fn new(repo: Arc<dyn CardRepository>) -> Self {
        Self { repo }
    }
}

impl LookAtCardsUseCase for DefaultLookAtCardsUseCase {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<GameCards, CardError>> {
        let repo = Arc::clone(&self.repo);
        tokio::spawn(async move {
            repo.game_cards(app_id)
                .await
                .map_err(|e| CardError::Unavailable(e.to_string()))
        })
    }
}

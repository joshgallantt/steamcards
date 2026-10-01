use std::sync::Arc;

use steam_library::AppId;
use tokio::task::JoinHandle;

use crate::{CardError, CardRepository, CardSet, LookAtFoilsUseCase};

pub struct DefaultLookAtFoilsUseCase {
    repo: Arc<dyn CardRepository>,
}

impl DefaultLookAtFoilsUseCase {
    pub fn new(repo: Arc<dyn CardRepository>) -> Self {
        Self { repo }
    }
}

impl LookAtFoilsUseCase for DefaultLookAtFoilsUseCase {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<CardSet, CardError>> {
        let repo = Arc::clone(&self.repo);
        tokio::spawn(async move {
            repo.foils(app_id)
                .await
                .map_err(|e| CardError::Unavailable(e.to_string()))
        })
    }
}

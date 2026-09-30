use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::{AssetId, CardAsset, CardError, CardRepository, IdentifyCardsUseCase};

pub struct DefaultIdentifyCardsUseCase {
    repo: Arc<dyn CardRepository>,
}

impl DefaultIdentifyCardsUseCase {
    pub fn new(repo: Arc<dyn CardRepository>) -> Self {
        Self { repo }
    }
}

impl IdentifyCardsUseCase for DefaultIdentifyCardsUseCase {
    fn call(&self, asset_ids: Vec<AssetId>) -> JoinHandle<Result<Vec<CardAsset>, CardError>> {
        let repo = Arc::clone(&self.repo);
        tokio::spawn(async move {
            repo.describe(&asset_ids)
                .await
                .map_err(|e| CardError::Unavailable(e.to_string()))
        })
    }
}

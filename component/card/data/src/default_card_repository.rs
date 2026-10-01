use std::sync::Arc;

use async_trait::async_trait;
use card::{AssetId, CardAsset, CardRepository, CardSet, GameCards};
use game::AppId;

use crate::CardClient;

/// The account's cards, as the client reads them.
pub struct DefaultCardRepository {
    client: Arc<dyn CardClient>,
}

impl DefaultCardRepository {
    pub fn new(client: Arc<dyn CardClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CardRepository for DefaultCardRepository {
    async fn game_cards(&self, app_id: AppId) -> anyhow::Result<GameCards> {
        self.client.game_cards(app_id).await
    }

    async fn foils(&self, app_id: AppId) -> anyhow::Result<CardSet> {
        self.client.foils(app_id).await
    }

    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>> {
        self.client.describe(asset_ids).await
    }
}

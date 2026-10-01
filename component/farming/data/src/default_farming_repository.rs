use std::sync::Arc;

use async_trait::async_trait;
use farming::{FarmingRepository, Signal};
use game::AppId;

use crate::FarmingClient;

/// Farming through the client.
pub struct DefaultFarmingRepository {
    client: Arc<dyn FarmingClient>,
}

impl DefaultFarmingRepository {
    pub fn new(client: Arc<dyn FarmingClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl FarmingRepository for DefaultFarmingRepository {
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()> {
        self.client.play(app_ids, online).await
    }

    async fn listen(&self) -> anyhow::Result<()> {
        self.client.listen().await
    }

    async fn stop(&self) {
        self.client.stop().await;
    }

    fn blocked(&self) -> Option<Option<AppId>> {
        self.client.blocked()
    }

    fn replaced(&self) -> bool {
        self.client.replaced()
    }

    async fn next_signal(&self) -> Signal {
        self.client.next_signal().await
    }
}

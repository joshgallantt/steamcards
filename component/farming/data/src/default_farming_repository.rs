use std::sync::Arc;

use async_trait::async_trait;
use farming::{FarmingRepository, Signal};
use keep_awake::KeepAwake;
use steam_library::AppId;

use crate::FarmingClient;

/// Farming through the client, with the computer kept awake while anything
/// is to be played.
pub struct DefaultFarmingRepository {
    client: Arc<dyn FarmingClient>,
    awake: Arc<KeepAwake>,
}

impl DefaultFarmingRepository {
    pub fn new(client: Arc<dyn FarmingClient>, awake: Arc<KeepAwake>) -> Self {
        Self { client, awake }
    }
}

#[async_trait]
impl FarmingRepository for DefaultFarmingRepository {
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()> {
        self.client.play(app_ids, online).await?;
        if app_ids.is_empty() {
            self.awake.let_sleep();
        } else {
            self.awake.hold();
        }
        Ok(())
    }

    async fn listen(&self) -> anyhow::Result<()> {
        self.client.listen().await
    }

    async fn stop(&self) {
        self.client.stop().await;
        self.awake.let_sleep();
    }

    fn blocked(&self) -> Option<Option<AppId>> {
        self.client.blocked()
    }

    async fn next_signal(&self) -> Signal {
        self.client.next_signal().await
    }
}

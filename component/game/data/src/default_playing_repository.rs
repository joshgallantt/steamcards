use std::sync::Arc;

use async_trait::async_trait;
use game::{AppId, Playing, PlayingRepository, PlayingSignal};

use crate::PlayingClient;

/// Playing, as the client does it.
pub struct DefaultPlayingRepository {
    client: Arc<dyn PlayingClient>,
}

impl DefaultPlayingRepository {
    pub fn new(client: Arc<dyn PlayingClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PlayingRepository for DefaultPlayingRepository {
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()> {
        self.client.play(app_ids, online).await
    }

    async fn listen(&self) -> anyhow::Result<()> {
        self.client.listen().await
    }

    async fn stop(&self) {
        self.client.stop().await;
    }

    fn playing(&self) -> Playing {
        match self.client.blocked() {
            Some(by) => Playing::Elsewhere(by),
            None => Playing::Here,
        }
    }

    async fn next_signal(&self) -> PlayingSignal {
        self.client.next_signal().await
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use game::{GameRepository, SteamLibrary};

use crate::GameClient;

/// The library, as the client reads it.
pub struct DefaultGameRepository {
    client: Arc<dyn GameClient>,
}

impl DefaultGameRepository {
    pub fn new(client: Arc<dyn GameClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl GameRepository for DefaultGameRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        Ok(SteamLibrary::new(self.client.games().await?))
    }

    fn replaced(&self) -> bool {
        self.client.replaced()
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use steam_library::{SteamLibrary, SteamLibraryRepository};

use crate::LibraryClient;

/// The library, as the client reads it.
pub struct DefaultSteamLibraryRepository {
    client: Arc<dyn LibraryClient>,
}

impl DefaultSteamLibraryRepository {
    pub fn new(client: Arc<dyn LibraryClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SteamLibraryRepository for DefaultSteamLibraryRepository {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        Ok(SteamLibrary::new(self.client.games().await?))
    }
}

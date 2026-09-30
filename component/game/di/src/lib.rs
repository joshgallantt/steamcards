//! Where the game domain meets its data layer: one repository over the
//! Steam session, handed to every use case. The composition root names the
//! Steam client.

use std::sync::Arc;

use game::{GameRepository, ReadLibrary, read_library};
use game_data::SteamGameRepository;
use steam_api::SteamClient;

pub struct GameComponent {
    pub read: ReadLibrary,
}

impl GameComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(Arc::new(SteamGameRepository::new(steam)))
    }

    pub fn over(repo: Arc<dyn GameRepository>) -> Self {
        Self {
            read: read_library(repo),
        }
    }
}

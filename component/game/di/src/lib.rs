//! Where the game domain meets its data layer: the library as Steam's badge
//! pages show it, through one repository handed to every use case. The
//! composition root names the Steam client.

use std::sync::Arc;

use game::{DefaultReadLibraryUseCase, GameRepository, ReadLibraryUseCase};
use game_data::{DefaultGameRepository, GameClient, SteamGameClient};
use steam_api::SteamClient;

pub struct GameComponent {
    pub read_library: Arc<dyn ReadLibraryUseCase>,
}

impl GameComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(Arc::new(SteamGameClient::new(steam)))
    }

    /// Over a client of its own: the repository is built here, and never let
    /// out.
    pub fn over(client: Arc<dyn GameClient>) -> Self {
        let repo: Arc<dyn GameRepository> = Arc::new(DefaultGameRepository::new(client));
        Self {
            read_library: Arc::new(DefaultReadLibraryUseCase::new(repo)),
        }
    }
}

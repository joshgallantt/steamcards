//! Where the game domain meets its data layer: one repository over the
//! Steam session, handed to every use case. The composition root names the
//! session.

use std::sync::Arc;

use game::{GameRepository, ReadLibrary, read_library};
use game_data::SteamGameRepository;
use steam_api::Session;

pub struct GameComponent {
    pub read: ReadLibrary,
}

impl GameComponent {
    pub fn new(session: Arc<Session>) -> Self {
        Self::over(Arc::new(SteamGameRepository::new(session)))
    }

    pub fn over(repo: Arc<dyn GameRepository>) -> Self {
        Self {
            read: read_library(repo),
        }
    }
}

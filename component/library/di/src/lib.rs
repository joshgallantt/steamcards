//! Where the library domain meets its data layer: one repository over the
//! Steam session, handed to every use case. The composition root names the
//! session.

use std::sync::Arc;

use library::{
    DescribeCards, LibraryRepository, LookAtGame, ReadLibrary, describe_cards, look_at_game,
    read_library,
};
use library_data::SteamLibraryRepository;
use steam_api::Session;

pub struct LibraryComponent {
    pub read: ReadLibrary,
    pub look_at: LookAtGame,
    pub describe: DescribeCards,
}

impl LibraryComponent {
    pub fn new(session: Arc<Session>) -> Self {
        Self::over(Arc::new(SteamLibraryRepository::new(session)))
    }

    pub fn over(repo: Arc<dyn LibraryRepository>) -> Self {
        Self {
            read: read_library(repo.clone()),
            look_at: look_at_game(repo.clone()),
            describe: describe_cards(repo),
        }
    }
}

//! Where the steam-library domain meets its data layer: the library as
//! Steam's badge pages show it, through one repository handed to every use
//! case. The composition root names the Steam client.

use std::sync::Arc;

use steam_api::SteamClient;
use steam_library::{DefaultReadLibraryUseCase, ReadLibraryUseCase, SteamLibraryRepository};
use steam_library_data::{DefaultSteamLibraryRepository, LibraryClient, SteamLibraryClient};

pub struct SteamLibraryComponent {
    pub read_library: Arc<dyn ReadLibraryUseCase>,
}

impl SteamLibraryComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(Arc::new(SteamLibraryClient::new(steam)))
    }

    /// Over a client of its own: the repository is built here, and never let
    /// out.
    pub fn over(client: Arc<dyn LibraryClient>) -> Self {
        let repo: Arc<dyn SteamLibraryRepository> =
            Arc::new(DefaultSteamLibraryRepository::new(client));
        Self {
            read_library: Arc::new(DefaultReadLibraryUseCase::new(repo)),
        }
    }
}

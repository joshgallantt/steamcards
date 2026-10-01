//! Where the game domain meets its data layer: the library as Steam's badge
//! pages show it through one repository, and playing on the Steam session's
//! CM connection through another, each handed to the use cases that need
//! it. The composition root names the Steam client.

use std::sync::Arc;

use game::{
    DefaultGetLibraryUseCase, DefaultObservePlayingUseCase, DefaultPlayGamesUseCase,
    DefaultStandByUseCase, DefaultStopPlayingUseCase, GameRepository, GetLibraryUseCase,
    ObservePlayingUseCase, PlayGamesUseCase, PlayingRepository, StandByUseCase, StopPlayingUseCase,
};
use game_data::{
    DefaultGameRepository, DefaultPlayingRepository, GameClient, PlayingClient, SteamGameClient,
    SteamPlayingClient,
};
use steam_api::SteamClient;

pub struct GameComponent {
    pub get_library: Arc<dyn GetLibraryUseCase>,
    pub play_games: Arc<dyn PlayGamesUseCase>,
    pub stand_by: Arc<dyn StandByUseCase>,
    pub stop_playing: Arc<dyn StopPlayingUseCase>,
    pub observe_playing: Arc<dyn ObservePlayingUseCase>,
}

impl GameComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(
            Arc::new(SteamGameClient::new(steam.clone())),
            Arc::new(SteamPlayingClient::new(steam)),
        )
    }

    /// Over clients of its own: the repositories are built here, and never
    /// let out.
    pub fn over(client: Arc<dyn GameClient>, playing: Arc<dyn PlayingClient>) -> Self {
        let repo: Arc<dyn GameRepository> = Arc::new(DefaultGameRepository::new(client));
        let playing: Arc<dyn PlayingRepository> = Arc::new(DefaultPlayingRepository::new(playing));
        Self {
            get_library: Arc::new(DefaultGetLibraryUseCase::new(repo)),
            play_games: Arc::new(DefaultPlayGamesUseCase::new(playing.clone())),
            stand_by: Arc::new(DefaultStandByUseCase::new(playing.clone())),
            stop_playing: Arc::new(DefaultStopPlayingUseCase::new(playing.clone())),
            observe_playing: Arc::new(DefaultObservePlayingUseCase::new(playing)),
        }
    }
}

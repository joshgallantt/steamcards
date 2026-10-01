use std::sync::Arc;

use card::{
    DefaultIdentifyCardsUseCase, DefaultLookAtCardsUseCase, DefaultLookAtFoilsUseCase,
    DefaultObserveNewItemsUseCase,
};
use game::{
    DefaultGetLibraryUseCase, DefaultObservePlayingUseCase, DefaultPlayGamesUseCase,
    DefaultStandByUseCase, DefaultStopPlayingUseCase,
};
use preferences::GetPreferencesUseCase;
use session::SessionKeeper;

use crate::{FarmingDependencies, test_support::FakeSteamAccount};

/// What farming works through, over `steam`: the game and card components'
/// use cases as the app has them, each over the stand-in, with these
/// preferences, and this keeper of the session.
pub fn farming_over(
    steam: &Arc<FakeSteamAccount>,
    get_preferences: Arc<dyn GetPreferencesUseCase>,
    sessions: Arc<SessionKeeper>,
) -> FarmingDependencies {
    FarmingDependencies {
        get_library: Arc::new(DefaultGetLibraryUseCase::new(steam.clone())),
        play_games: Arc::new(DefaultPlayGamesUseCase::new(steam.clone())),
        stand_by: Arc::new(DefaultStandByUseCase::new(steam.clone())),
        stop_playing: Arc::new(DefaultStopPlayingUseCase::new(steam.clone())),
        observe_playing: Arc::new(DefaultObservePlayingUseCase::new(steam.clone())),
        look_at_cards: Arc::new(DefaultLookAtCardsUseCase::new(steam.clone())),
        look_at_foils: Arc::new(DefaultLookAtFoilsUseCase::new(steam.clone())),
        identify_cards: Arc::new(DefaultIdentifyCardsUseCase::new(steam.clone())),
        observe_new_items: Arc::new(DefaultObserveNewItemsUseCase::new(steam.clone())),
        get_preferences,
        sessions,
    }
}

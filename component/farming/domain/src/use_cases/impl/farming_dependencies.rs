use std::sync::Arc;

use card::{IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase, ObserveNewItemsUseCase};
use game::{
    GetLibraryUseCase, ObservePlayingUseCase, PlayGamesUseCase, StandByUseCase, StopPlayingUseCase,
};
use preferences::GetPreferencesUseCase;
use session::SessionKeeper;

/// What the farmer works through: the game, card and preferences
/// components' use cases, never their storage, and the keeper of the
/// session it writes. Farming keeps nothing of its own.
pub struct FarmingDependencies {
    pub get_library: Arc<dyn GetLibraryUseCase>,
    pub play_games: Arc<dyn PlayGamesUseCase>,
    pub stand_by: Arc<dyn StandByUseCase>,
    pub stop_playing: Arc<dyn StopPlayingUseCase>,
    pub observe_playing: Arc<dyn ObservePlayingUseCase>,
    pub look_at_cards: Arc<dyn LookAtCardsUseCase>,
    pub look_at_foils: Arc<dyn LookAtFoilsUseCase>,
    pub identify_cards: Arc<dyn IdentifyCardsUseCase>,
    pub observe_new_items: Arc<dyn ObserveNewItemsUseCase>,
    pub get_preferences: Arc<dyn GetPreferencesUseCase>,
    pub sessions: Arc<SessionKeeper>,
}

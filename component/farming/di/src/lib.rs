//! Where the farming domain meets its data layer: games played on the Steam
//! session through a client. Farming asks the steam-library, card and
//! preferences components through their use cases, never their storage; the
//! composition root hands them in, and the keeper of the session the farmer
//! writes.

use std::sync::Arc;

use card::{IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase};
use farming::{DefaultFarmCardsUseCase, FarmCardsUseCase};
use farming_data::{DefaultFarmingRepository, FarmingClient, SteamFarmingClient};
use preferences::GetPreferencesUseCase;
use session::SessionKeeper;
use steam_api::SteamClient;
use steam_library::ReadLibraryUseCase;

pub struct FarmingComponent {
    pub farm_cards: Arc<dyn FarmCardsUseCase>,
}

impl FarmingComponent {
    pub fn new(
        steam: Arc<SteamClient>,
        read_library: Arc<dyn ReadLibraryUseCase>,
        look_at_cards: Arc<dyn LookAtCardsUseCase>,
        look_at_foils: Arc<dyn LookAtFoilsUseCase>,
        identify_cards: Arc<dyn IdentifyCardsUseCase>,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        sessions: Arc<SessionKeeper>,
    ) -> Self {
        Self::over(
            Arc::new(SteamFarmingClient::new(steam)),
            read_library,
            look_at_cards,
            look_at_foils,
            identify_cards,
            get_preferences,
            sessions,
        )
    }

    /// Over a client of its own: the repository is built here, and never let
    /// out.
    pub fn over(
        client: Arc<dyn FarmingClient>,
        read_library: Arc<dyn ReadLibraryUseCase>,
        look_at_cards: Arc<dyn LookAtCardsUseCase>,
        look_at_foils: Arc<dyn LookAtFoilsUseCase>,
        identify_cards: Arc<dyn IdentifyCardsUseCase>,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        sessions: Arc<SessionKeeper>,
    ) -> Self {
        Self {
            farm_cards: Arc::new(DefaultFarmCardsUseCase::new(
                read_library,
                look_at_cards,
                look_at_foils,
                identify_cards,
                Arc::new(DefaultFarmingRepository::new(client)),
                get_preferences,
                sessions,
            )),
        }
    }
}

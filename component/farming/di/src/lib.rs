//! Where the farming domain meets its data layer: games played on the Steam
//! session through a client, with the computer kept awake meanwhile. Farming
//! asks the game, card and preferences components through their use cases,
//! never their storage; the composition root hands them in, and the keeper
//! of the session the farmer writes.

use std::sync::Arc;

use card::{IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase};
use farming::{DefaultFarmCardsUseCase, FarmCardsUseCase};
use farming_data::{DefaultFarmingRepository, FarmingClient, SteamFarmingClient};
use game::ReadLibraryUseCase;
use keep_awake::KeepAwake;
use preferences::GetPreferencesUseCase;
use session::SessionKeeper;
use steam_api::SteamClient;

pub struct FarmingComponent {
    pub farm_cards: Arc<dyn FarmCardsUseCase>,
}

impl FarmingComponent {
    #[expect(
        clippy::too_many_arguments,
        reason = "the farmer runs on the Steam client, the computer staying awake and the \
                  session's keeper, and on five use cases of three components, each handed \
                  in on its own rather than as a whole container"
    )]
    pub fn new(
        steam: Arc<SteamClient>,
        awake: Arc<KeepAwake>,
        read_library: Arc<dyn ReadLibraryUseCase>,
        look_at_cards: Arc<dyn LookAtCardsUseCase>,
        look_at_foils: Arc<dyn LookAtFoilsUseCase>,
        identify_cards: Arc<dyn IdentifyCardsUseCase>,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        sessions: Arc<SessionKeeper>,
    ) -> Self {
        Self::over(
            Arc::new(SteamFarmingClient::new(steam)),
            awake,
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
    #[expect(
        clippy::too_many_arguments,
        reason = "the farmer plays through the client, keeping the computer awake, and writes \
                  the session's keeper, on five use cases of three components, each handed in \
                  on its own rather than as a whole container"
    )]
    pub fn over(
        client: Arc<dyn FarmingClient>,
        awake: Arc<KeepAwake>,
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
                Arc::new(DefaultFarmingRepository::new(client, awake)),
                get_preferences,
                sessions,
            )),
        }
    }
}

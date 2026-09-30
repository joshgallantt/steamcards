//! Where the farming domain meets its data layer: games played on the Steam
//! session, with the computer kept awake meanwhile. Farming asks the game,
//! card and preferences components through their use cases, never their storage;
//! the composition root hands them in, and the keeper of the session the
//! farmer writes.

use std::sync::Arc;

use card::{IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase};
use farming::{DefaultFarmCardsUseCase, FarmCardsUseCase, FarmingRepository};
use farming_data::SteamFarmingRepository;
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
            Arc::new(SteamFarmingRepository::new(steam, awake)),
            read_library,
            look_at_cards,
            look_at_foils,
            identify_cards,
            get_preferences,
            sessions,
        )
    }

    pub fn over(
        play: Arc<dyn FarmingRepository>,
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
                play,
                get_preferences,
                sessions,
            )),
        }
    }
}

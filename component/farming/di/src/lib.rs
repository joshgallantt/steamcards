//! Where the farming domain meets its data layer: games played on the Steam
//! session, with the computer kept awake meanwhile. Farming asks the library and preferences components through
//! their use cases, never their storage; the composition root hands them in.

use std::sync::Arc;

use farming::{FarmCards, PlayRepository, farm_cards};
use farming_data::SteamPlayRepository;
use keep_awake::KeepAwake;
use library::{LookAtGame, ReadLibrary};
use preferences::GetPreferences;
use steam_api::Session;

pub struct FarmingComponent {
    pub farm: FarmCards,
}

impl FarmingComponent {
    pub fn new(
        session: Arc<Session>,
        awake: Arc<KeepAwake>,
        read_library: ReadLibrary,
        look_at_game: LookAtGame,
        get_preferences: GetPreferences,
    ) -> Self {
        Self::over(
            Arc::new(SteamPlayRepository::new(session, awake)),
            read_library,
            look_at_game,
            get_preferences,
        )
    }

    pub fn over(
        play: Arc<dyn PlayRepository>,
        read_library: ReadLibrary,
        look_at_game: LookAtGame,
        get_preferences: GetPreferences,
    ) -> Self {
        Self {
            farm: farm_cards(read_library, look_at_game, play, get_preferences),
        }
    }
}

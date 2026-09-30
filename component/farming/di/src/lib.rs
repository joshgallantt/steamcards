//! Where the farming domain meets its data layer: games played on the Steam
//! session, with the computer kept awake meanwhile. Farming asks the game,
//! card and preferences components through their use cases, never their storage;
//! the composition root hands them in, and the keeper of the session the
//! farmer writes.

use std::sync::Arc;

use card::{DescribeCards, LookAtCards, LookAtFoils};
use farming::{FarmCards, PlayRepository, farm_cards};
use farming_data::SteamPlayRepository;
use game::ReadLibrary;
use keep_awake::KeepAwake;
use preferences::GetPreferences;
use session::SessionKeeper;
use steam_api::SteamClient;

pub struct FarmingComponent {
    pub farm: FarmCards,
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
        read_library: ReadLibrary,
        look_at_cards: LookAtCards,
        look_at_foils: LookAtFoils,
        describe_cards: DescribeCards,
        get_preferences: GetPreferences,
        sessions: Arc<SessionKeeper>,
    ) -> Self {
        Self::over(
            Arc::new(SteamPlayRepository::new(steam, awake)),
            read_library,
            look_at_cards,
            look_at_foils,
            describe_cards,
            get_preferences,
            sessions,
        )
    }

    pub fn over(
        play: Arc<dyn PlayRepository>,
        read_library: ReadLibrary,
        look_at_cards: LookAtCards,
        look_at_foils: LookAtFoils,
        describe_cards: DescribeCards,
        get_preferences: GetPreferences,
        sessions: Arc<SessionKeeper>,
    ) -> Self {
        Self {
            farm: farm_cards(
                read_library,
                look_at_cards,
                look_at_foils,
                describe_cards,
                play,
                get_preferences,
                sessions,
            ),
        }
    }
}

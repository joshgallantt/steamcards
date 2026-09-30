//! Where the farming domain meets its data layer: games played on the Steam
//! session, with the computer kept awake meanwhile. Farming asks the library
//! and preferences components through their use cases, never their storage;
//! the composition root hands them in. Farming and ending its session share
//! one keeper of the session.

use std::sync::Arc;

use farming::{EndSession, FarmCards, PlayRepository, SessionKeeper, end_session, farm_cards};
use farming_data::SteamPlayRepository;
use keep_awake::KeepAwake;
use library::{DescribeCards, LookAtFoils, LookAtGame, ReadLibrary};
use preferences::GetPreferences;
use steam_api::Session;

pub struct FarmingComponent {
    pub farm: FarmCards,
    pub end_session: EndSession,
}

impl FarmingComponent {
    pub fn new(
        session: Arc<Session>,
        awake: Arc<KeepAwake>,
        read_library: ReadLibrary,
        look_at_game: LookAtGame,
        look_at_foils: LookAtFoils,
        describe_cards: DescribeCards,
        get_preferences: GetPreferences,
    ) -> Self {
        Self::over(
            Arc::new(SteamPlayRepository::new(session, awake)),
            read_library,
            look_at_game,
            look_at_foils,
            describe_cards,
            get_preferences,
        )
    }

    pub fn over(
        play: Arc<dyn PlayRepository>,
        read_library: ReadLibrary,
        look_at_game: LookAtGame,
        look_at_foils: LookAtFoils,
        describe_cards: DescribeCards,
        get_preferences: GetPreferences,
    ) -> Self {
        let sessions = Arc::new(SessionKeeper::default());
        Self {
            farm: farm_cards(
                read_library,
                look_at_game,
                look_at_foils,
                describe_cards,
                play,
                get_preferences,
                sessions.clone(),
            ),
            end_session: end_session(sessions),
        }
    }
}

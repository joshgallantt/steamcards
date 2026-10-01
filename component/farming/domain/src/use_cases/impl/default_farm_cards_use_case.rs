use std::sync::Arc;

use card::{IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase};
use game::GetLibraryUseCase;
use preferences::GetPreferencesUseCase;
use session::SessionKeeper;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{FarmCardsUseCase, FarmingRepository, FarmingUpdate, farmer::Farmer};

pub struct DefaultFarmCardsUseCase {
    farmer: Arc<Farmer>,
}

impl DefaultFarmCardsUseCase {
    pub fn new(
        get_library: Arc<dyn GetLibraryUseCase>,
        look_at_cards: Arc<dyn LookAtCardsUseCase>,
        look_at_foils: Arc<dyn LookAtFoilsUseCase>,
        identify_cards: Arc<dyn IdentifyCardsUseCase>,
        play: Arc<dyn FarmingRepository>,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        sessions: Arc<SessionKeeper>,
    ) -> Self {
        Self {
            farmer: Arc::new(Farmer::new(
                get_library,
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

impl FarmCardsUseCase for DefaultFarmCardsUseCase {
    fn call(
        &self,
        token: CancellationToken,
        updates: mpsc::Sender<FarmingUpdate>,
    ) -> JoinHandle<()> {
        let farmer = Arc::clone(&self.farmer);
        tokio::spawn(async move { farmer.farm(token, updates).await })
    }
}

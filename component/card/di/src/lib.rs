//! Where the card domain meets its data layer: one repository over the
//! Steam session, handed to every use case. The composition root names the
//! Steam client.

use std::sync::Arc;

use card::{
    CardRepository, DefaultIdentifyCardsUseCase, DefaultLookAtCardsUseCase,
    DefaultLookAtFoilsUseCase, IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase,
};
use card_data::SteamCardRepository;
use steam_api::SteamClient;

pub struct CardComponent {
    pub look_at_cards: Arc<dyn LookAtCardsUseCase>,
    pub look_at_foils: Arc<dyn LookAtFoilsUseCase>,
    pub identify_cards: Arc<dyn IdentifyCardsUseCase>,
}

impl CardComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(Arc::new(SteamCardRepository::new(steam)))
    }

    pub fn over(repo: Arc<dyn CardRepository>) -> Self {
        Self {
            look_at_cards: Arc::new(DefaultLookAtCardsUseCase::new(repo.clone())),
            look_at_foils: Arc::new(DefaultLookAtFoilsUseCase::new(repo.clone())),
            identify_cards: Arc::new(DefaultIdentifyCardsUseCase::new(repo)),
        }
    }
}

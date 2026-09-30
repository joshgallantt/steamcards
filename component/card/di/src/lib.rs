//! Where the card domain meets its data layer: the cards as Steam shows them,
//! through one repository handed to every use case. The composition root
//! names the Steam client.

use std::sync::Arc;

use card::{
    CardRepository, DefaultIdentifyCardsUseCase, DefaultLookAtCardsUseCase,
    DefaultLookAtFoilsUseCase, IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase,
};
use card_data::{CardClient, DefaultCardRepository, SteamCardClient};
use steam_api::SteamClient;

pub struct CardComponent {
    pub look_at_cards: Arc<dyn LookAtCardsUseCase>,
    pub look_at_foils: Arc<dyn LookAtFoilsUseCase>,
    pub identify_cards: Arc<dyn IdentifyCardsUseCase>,
}

impl CardComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(Arc::new(SteamCardClient::new(steam)))
    }

    /// Over a client of its own: the repository is built here, and never let
    /// out.
    pub fn over(client: Arc<dyn CardClient>) -> Self {
        let repo: Arc<dyn CardRepository> = Arc::new(DefaultCardRepository::new(client));
        Self {
            look_at_cards: Arc::new(DefaultLookAtCardsUseCase::new(repo.clone())),
            look_at_foils: Arc::new(DefaultLookAtFoilsUseCase::new(repo.clone())),
            identify_cards: Arc::new(DefaultIdentifyCardsUseCase::new(repo)),
        }
    }
}

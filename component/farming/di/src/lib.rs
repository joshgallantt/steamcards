//! Where farming is put together. The farmer keeps nothing of its own, so
//! farming has no data layer: it plays and looks through the game and card
//! components' use cases, and asks the preferences component's what the
//! user wants. The composition root hands those in, and the keeper of the
//! session the farmer writes.

use std::sync::Arc;

use farming::{DefaultFarmCardsUseCase, FarmCardsUseCase, FarmingDependencies};

pub struct FarmingComponent {
    pub farm_cards: Arc<dyn FarmCardsUseCase>,
}

impl FarmingComponent {
    pub fn new(dependencies: FarmingDependencies) -> Self {
        Self {
            farm_cards: Arc::new(DefaultFarmCardsUseCase::new(dependencies)),
        }
    }
}

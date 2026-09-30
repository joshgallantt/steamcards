//! Where the card domain meets its data layer: one repository over the
//! Steam session, handed to every use case. The composition root names the
//! session.

use std::sync::Arc;

use card::{
    CardRepository, DescribeCards, LookAtCards, LookAtFoils, describe_cards, look_at_cards,
    look_at_foils,
};
use card_data::SteamCardRepository;
use steam_api::Session;

pub struct CardComponent {
    pub look_at: LookAtCards,
    pub look_at_foils: LookAtFoils,
    pub describe: DescribeCards,
}

impl CardComponent {
    pub fn new(session: Arc<Session>) -> Self {
        Self::over(Arc::new(SteamCardRepository::new(session)))
    }

    pub fn over(repo: Arc<dyn CardRepository>) -> Self {
        Self {
            look_at: look_at_cards(repo.clone()),
            look_at_foils: look_at_foils(repo.clone()),
            describe: describe_cards(repo),
        }
    }
}

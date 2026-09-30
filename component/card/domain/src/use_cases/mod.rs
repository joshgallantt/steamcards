mod card_use_cases;
mod r#impl;

pub use card_use_cases::{IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase};
pub use r#impl::{
    DefaultIdentifyCardsUseCase, DefaultLookAtCardsUseCase, DefaultLookAtFoilsUseCase,
};

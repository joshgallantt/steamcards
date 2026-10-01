//! Card: the trading cards of the user's games. Each card comes in two
//! kinds, normal and foil, and each kind makes a badge of its own: so a
//! game's set, of either kind, and how many of each card the account has;
//! and the copies of cards the account holds, each an item in its
//! inventory. A card is from the set of a game in the [`steam_library`], so
//! this component uses that one.
//!
//! How Steam is asked (card pages, foils' pages, the inventory) is the data
//! layer's business, behind [`CardRepository`]. Farming looks at cards with
//! the use cases here, never the repository.

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{AssetId, Card, CardAsset, CardError, CardKind, CardSet, CardSets, GameCards};
pub use repository::CardRepository;
pub use use_cases::{
    DefaultIdentifyCardsUseCase, DefaultLookAtCardsUseCase, DefaultLookAtFoilsUseCase,
    IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase,
};

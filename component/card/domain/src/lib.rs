//! Card: the trading cards of the user's games. Each game's set, and how
//! many of each card the account has; its foils, which make a badge of
//! their own; and the copies of cards the account holds, each an item in
//! its inventory. A card is from a [`game`]'s set, so this component uses
//! that one.
//!
//! How Steam is asked (card pages, foils' pages, the inventory) is the data
//! layer's business, behind [`CardRepository`]. Farming looks at cards with
//! the use cases here, never the repository.

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{Card, CardAsset, CardError, CardSet, CardSets, GameCards};
pub use repository::CardRepository;
pub use use_cases::{
    DescribeCards, LookAtCards, LookAtFoils, describe_cards, look_at_cards, look_at_foils,
};

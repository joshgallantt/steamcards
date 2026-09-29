//! Library: the user's Steam library, as far as trading cards go — the games
//! that have cards, the cards in their sets, the drops still to come, and
//! the copies of cards the account holds.
//!
//! How Steam is asked (badge pages, card pages, the inventory) is the data
//! layer's business, behind [`LibraryRepository`]. Farming works through the
//! library with the use cases here, never the repository.

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{Card, CardAsset, CardDrops, Game, LibraryError, SteamLibrary};
pub use repository::LibraryRepository;
pub use use_cases::{
    DescribeCards, LookAtGame, ReadLibrary, describe_cards, look_at_game, read_library,
};

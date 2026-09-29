//! Library: the user's Steam library, as far as trading cards go — the games
//! that have cards, the cards in their sets, and the drops still to come.
//!
//! How Steam is asked (badge pages, card pages) is the data layer's
//! business, behind [`LibraryRepository`]. Farming works through the library
//! with the use cases here, never the repository.

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{Card, CardDrops, Game, LibraryError, SteamLibrary};
pub use repository::LibraryRepository;
pub use use_cases::{LookAtGame, ReadLibrary, look_at_game, read_library};

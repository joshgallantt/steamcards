//! Game: the user's games that have trading cards, as far as farming goes —
//! each game's hours and badge, the card drops it has given and still has
//! to give, and the library of them all. What a game's cards are, its set
//! and the copies the account holds, is the card component's.
//!
//! How Steam is asked (the badge pages) is the data layer's business, behind
//! [`GameRepository`]. Farming works through the library with the use cases
//! here, never the repository.

mod model;
mod repository;
mod rules;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{AppId, CardDrops, Game, GameError, SteamLibrary};
pub use repository::GameRepository;
pub use rules::{HOURS_BEFORE_DROPS, MOST_PLAYED_AT_ONCE};
pub use use_cases::{DefaultReadLibraryUseCase, ReadLibraryUseCase};

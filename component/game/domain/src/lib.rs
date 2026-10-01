//! Game: the user's games that have trading cards, as far as farming goes,
//! each with its hours and badge, and the card drops it has given and still
//! has to give; and the Steam library of them all. What a game's cards are,
//! its set and the copies the account holds, is the card component's.
//!
//! Playing them is the game's too: playing exactly the games asked, or
//! standing by while another device plays, and hearing what Steam says of
//! it.
//!
//! How Steam is asked (the badge pages, the CM connection) is the data
//! layer's business, behind [`GameRepository`] and [`PlayingRepository`].
//! Farming reads the library and plays through the use cases here, never
//! the repositories.

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{
    AppId, CardDrops, Game, GameError, HOURS_BEFORE_DROPS, MOST_PLAYED_AT_ONCE, Playing,
    PlayingSignal, SteamLibrary,
};
pub use repository::{GameRepository, PlayingRepository};
pub use use_cases::{
    DefaultGetLibraryUseCase, DefaultObservePlayingUseCase, DefaultPlayGamesUseCase,
    DefaultStandByUseCase, DefaultStopPlayingUseCase, GetLibraryUseCase, ObservePlayingUseCase,
    PlayGamesUseCase, StandByUseCase, StopPlayingUseCase,
};

//! Farming: which of the user's games still drop cards, and playing them
//! until they don't.
//!
//! The rules of what gets played — ordering, playing several games at once
//! to build hours, one at a time for cards, waiting while another device
//! plays — live here once. How Steam is asked is the data layer's business,
//! behind [`CardsRepository`] and [`PlayRepository`]. What the user wants
//! comes from the `preferences` component, through its use case rather than
//! its storage.

mod listing;
mod model;
mod repository;

pub use listing::{ListGames, list_games};
pub use model::{EventKind, FarmingEvent, FarmingStatus, Game, Library, Mode, Signal, Status};
pub use repository::{CardsRepository, PlayRepository};

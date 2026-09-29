//! Farming: playing the user's games so their trading cards drop — which
//! games, in what order, one at a time or together to build hours, and
//! stepping aside while another device plays.
//!
//! It works through the [`library`] component's use cases (what can still
//! drop) and the [`preferences`] component's (what the user wants first),
//! never their storage. Playing is the data layer's business, behind
//! [`PlayRepository`]. The numbers it runs on, and where each comes from,
//! are in `rules.rs`.

mod model;
mod ranking;
mod reporter;
mod repository;
mod rules;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{EventKind, FarmingEvent, FarmingStatus, Mode, Signal, Status};
pub use repository::PlayRepository;
pub use use_cases::{FarmCards, farm_cards};

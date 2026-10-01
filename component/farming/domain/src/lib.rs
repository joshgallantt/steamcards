//! Farming: playing the user's games so their trading cards drop — which
//! games, in what order, one at a time or together to build hours, and
//! stepping aside while another device plays — writing the [`session`] it
//! makes as it goes.
//!
//! It works through the [`steam_library`] and [`card`] components' use cases (what
//! can still drop, a game's set, and which card an item is) and the
//! [`preferences`] component's (what the user wants first), never their
//! storage. Playing is the data layer's business, behind [`FarmingRepository`].
//! The numbers it runs on, and where each comes from, are in `rules.rs`.
//!
//! It says what happened ([`FarmingEvent`]) and where farming stands
//! ([`FarmingStatus`]), never how to put it: the screens choose the words.

mod farmer;
mod model;
mod ranking;
mod reporter;
mod repository;
mod rules;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{
    FarmingEvent, FarmingStatus, FarmingUpdate, NothingToFarm, Signal, Status, Trouble,
};
pub use ranking::farm_order;
pub use repository::FarmingRepository;
pub use use_cases::{DefaultFarmCardsUseCase, FarmCardsUseCase};

//! Farming: playing the user's games so their trading cards drop — which
//! games, in what order, one at a time or together to build hours, and
//! stepping aside while another device plays — writing the [`session`] it
//! makes as it goes.
//!
//! It works through the [`game`] component's use cases (what can still
//! drop, and playing it), the [`card`] component's (a game's set, new items,
//! and which card each is) and the [`preferences`] component's (what the
//! user wants first), never their storage, all handed in as
//! [`FarmingDependencies`]. It keeps nothing of its own, so it has no
//! repository, and no data layer. The numbers it runs on, and where each
//! comes from, are in `model/rules.rs`.
//!
//! It says what happened ([`FarmingEvent`]) and where farming stands
//! ([`FarmingStatus`]), never how to put it: the screens choose the words.

mod model;
mod service;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{
    FarmingEvent, FarmingStatus, FarmingUpdate, LeftOut, NothingToFarm, Status, Trouble,
};
pub use service::{farm_order, left_out};
pub use use_cases::{DefaultFarmCardsUseCase, FarmCardsUseCase, FarmingDependencies};

//! Farming: playing the user's games so their trading cards drop — which
//! games, in what order, one at a time or together to build hours, and
//! stepping aside while another device plays — and the session it makes:
//! every card that dropped and which it was, what was played, and how long
//! the rest should take.
//!
//! It works through the [`library`] component's use cases (what can still
//! drop, and which card an item is) and the [`preferences`] component's
//! (what the user wants first), never their storage. Playing is the data
//! layer's business, behind [`PlayRepository`]. The numbers it runs on, and
//! where each comes from, are in `rules.rs`.

mod forecast;
mod model;
mod ranking;
mod reporter;
mod repository;
mod rules;
mod session;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use forecast::forecast;
pub use model::{
    Drop, DropCard, EventKind, FarmingEvent, FarmingSession, FarmingStatus, Finished, Forecast,
    Mode, NewItem, SetAside, Signal, Status, Stretch,
};
pub use ranking::{farm_order, hours_to_go};
pub use repository::PlayRepository;
pub use session::SessionKeeper;
pub use use_cases::{EndSession, FarmCards, end_session, farm_cards};

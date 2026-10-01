//! Session: this session of farming, from the first run of the farmer until
//! the user signs out, another account signs in, or steamcards quits,
//! through pauses. Every card that dropped, one drop per copy, and which
//! card each was; what was played, and how; the games finished; and how long
//! the rest should take.
//!
//! The farmer writes it, and says which games it will farm and in what
//! order: that's the farmer's to decide, so it's handed in. A session is
//! kept in memory alone, so there's no data layer.

mod model;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{
    Drop, DropCard, Finished, Forecast, Found, KeptSession, Looked, Mode, Session, SessionKeeper,
    SetAside, Stretch,
};
pub use use_cases::{DefaultEndSessionUseCase, EndSessionUseCase};

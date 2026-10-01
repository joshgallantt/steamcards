//! Doubles for this crate's tests and other crates' tests, a file each, so
//! no test needs Steam: fakes work as the real thing would, in memory,
//! spies count what they're asked, and builders make what a test needs.

mod builders;
mod fakes;
mod spies;

pub use builders::farming_over;
pub use fakes::FakeSteamAccount;
pub use spies::SpyFarmCardsUseCase;

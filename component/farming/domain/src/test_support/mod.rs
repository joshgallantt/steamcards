//! Doubles for this crate's tests and other crates' tests, a file each, so
//! no test needs Steam: fakes work as the real thing would, in memory, and
//! spies count what they're asked.

mod fakes;
mod spies;

pub use fakes::FakeSteamAccount;
pub use spies::SpyFarmCardsUseCase;

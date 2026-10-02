//! Doubles for this crate's tests and other crates' tests, a file each, so
//! no test needs GitHub: fakes work as the real thing would, in memory, and
//! spies count what they're asked.

mod fakes;
mod spies;

pub use fakes::FakeReleaseRepository;
pub use spies::SpyKeepUpToDateUseCase;

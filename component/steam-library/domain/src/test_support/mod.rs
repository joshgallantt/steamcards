//! Doubles for other crates' tests, a file each, so no test needs Steam:
//! fakes work as the real thing would, in memory; spies count what they're
//! asked; builders make what a test needs.

mod builders;
mod fakes;
mod spies;

pub use builders::game;
pub use fakes::FakeSteamLibraryRepository;
pub use spies::SpyReadLibraryUseCase;

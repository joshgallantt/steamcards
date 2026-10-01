//! Doubles for other crates' tests, a file each, so no test needs a disk:
//! fakes work as the real thing would, in memory; stubs answer as they're
//! told.

mod fakes;
mod stubs;

pub use fakes::FakePreferencesRepository;
pub use stubs::StubGetPreferencesUseCase;

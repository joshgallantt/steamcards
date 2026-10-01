//! Doubles for other crates' tests, a file each: fakes work as the real
//! thing would, in memory; stubs answer as they're told; spies count what
//! they're asked.

mod fakes;
mod spies;
mod stubs;

pub use fakes::FakeAccountRepository;
pub use spies::{SpyCheckSignInUseCase, SpySignOutUseCase};
pub use stubs::{StubGetAccountUseCase, StubSignInUseCase};

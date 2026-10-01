//! Doubles for this crate's tests and other crates' tests, a file each, so
//! no test needs Steam: fakes work as the real thing would, in memory; stubs
//! answer as they're told; spies count what they're asked; builders make
//! what a test needs.

mod builders;
mod fakes;
mod spies;
mod stubs;

pub use builders::{
    clock_from, listing, order_book, pounds, priced_card, session_start, set_prices,
};
pub use fakes::{FakePriceRepository, SetLookup};
pub use spies::{SpyKeepPricesUpToDateUseCase, SpyRefreshPricesUseCase, SpySetGamesToPriceUseCase};
pub use stubs::{StubGetPricesUseCase, StubGetWalletUseCase};

//! Doubles for other crates' tests, a file each, so no test needs Steam:
//! fakes work as the real thing would, in memory; builders make what a test
//! needs.

mod builders;
mod fakes;

pub use builders::{card, card_asset};
pub use fakes::FakeCardRepository;

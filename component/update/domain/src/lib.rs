//! Update: steamcards' own releases, and keeping this copy up to date with
//! them. A copy the install scripts put in place puts a new release in place
//! itself, for the next start; one Homebrew or cargo looks after is told of
//! the new release, and how to get it. It looks when steamcards starts, and
//! once a day after, while the user leaves automatic updates on, as they are
//! at first.
//!
//! Where releases come from (GitHub), and how one is put in place, is the
//! data layer's business, behind [`ReleaseRepository`].

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{UpdateEvent, UpdatedBy, Version};
pub use repository::ReleaseRepository;
pub use use_cases::{DefaultKeepUpToDateUseCase, KeepUpToDateUseCase};

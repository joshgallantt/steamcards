//! Preferences: what the user wants farmed first, what never, and whether
//! they show as online while it runs.
//!
//! The domain of this component — entities, the repository contract the data
//! layer is written to fit, and one use case per thing the user can change.
//! Depends on nothing but `anyhow`, which a repository uses to say *that* a
//! write failed; the use cases translate that into [`PreferencesError`].

mod model;
mod repository;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{Preferences, PreferencesError, Tier};
pub use repository::PreferencesRepository;
pub use use_cases::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetGameTierUseCase,
    DefaultSetOnlyPriorityUseCase, GetPreferencesUseCase, SetAppearOnlineUseCase,
    SetGameTierUseCase, SetOnlyPriorityUseCase,
};

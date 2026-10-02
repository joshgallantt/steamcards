//! Preferences: what the user wants farmed first, what never, and how:
//! whether they show as online while it runs, the hours their account holds
//! cards back for, and whether private games, and games Steam would still
//! refund, are left out.
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
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetAutoUpdateUseCase,
    DefaultSetGameTierUseCase, DefaultSetHoursBeforeDropsUseCase, DefaultSetOnlyPriorityUseCase,
    DefaultSetRestartGamesUseCase, DefaultSetSkipPrivateUseCase, DefaultSetSkipRefundableUseCase,
    GetPreferencesUseCase, SetAppearOnlineUseCase, SetAutoUpdateUseCase, SetGameTierUseCase,
    SetHoursBeforeDropsUseCase, SetOnlyPriorityUseCase, SetRestartGamesUseCase,
    SetSkipPrivateUseCase, SetSkipRefundableUseCase,
};

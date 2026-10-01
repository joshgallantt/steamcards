//! Everything the user can change about what's farmed, a trait each. Each is
//! done over the repository by a `Default…UseCase` in `impl/`. A view model
//! holds only the ones it calls, and a test double is a type of its own in
//! `test_support`.

use game::AppId;

use crate::{Preferences, PreferencesError, Tier};

/// The current preferences.
pub trait GetPreferencesUseCase: Send + Sync {
    fn call(&self) -> Preferences;
}

/// Moves a game (by app ID) between tiers. `Tier::Priority(n)` puts it at
/// position n (clamped to the end of the list), shifting the others down.
pub trait SetGameTierUseCase: Send + Sync {
    fn call(&self, app_id: AppId, tier: Tier) -> Result<(), PreferencesError>;
}

/// Farms priority games only, or everything not skipped.
pub trait SetOnlyPriorityUseCase: Send + Sync {
    fn call(&self, only: bool) -> Result<(), PreferencesError>;
}

/// Shows as online to friends while farming, or appears offline.
pub trait SetAppearOnlineUseCase: Send + Sync {
    fn call(&self, online: bool) -> Result<(), PreferencesError>;
}

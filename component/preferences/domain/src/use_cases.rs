//! One function per thing the user can change. Each use case is a value: its
//! type says what it takes and gives, and a constructor builds the real one
//! over the repository. A view model holds only the ones it calls, and a test
//! double is just a closure.

use std::sync::Arc;

use crate::{Preferences, PreferencesError, PreferencesRepository, Tier};

/// The current preferences.
pub type GetPreferences = Arc<dyn Fn() -> Preferences + Send + Sync>;

/// Moves a game (by app ID) between tiers. `Tier::Priority(n)` puts it at
/// position n (clamped to the end of the list), shifting the others down.
pub type SetGameTier = Arc<dyn Fn(u32, Tier) -> Result<(), PreferencesError> + Send + Sync>;

/// Farms priority games only, or everything not skipped.
pub type SetOnlyPriority = Arc<dyn Fn(bool) -> Result<(), PreferencesError> + Send + Sync>;

/// Shows as online to friends while farming, or appears offline.
pub type SetAppearOnline = Arc<dyn Fn(bool) -> Result<(), PreferencesError> + Send + Sync>;

fn save(repo: &dyn PreferencesRepository, p: Preferences) -> Result<(), PreferencesError> {
    repo.save(p).map_err(|_| PreferencesError::Unavailable)
}

pub fn get_preferences(repo: Arc<dyn PreferencesRepository>) -> GetPreferences {
    Arc::new(move || repo.preferences())
}

pub fn set_game_tier(repo: Arc<dyn PreferencesRepository>) -> SetGameTier {
    Arc::new(move |app_id, tier| {
        let mut p = repo.preferences();
        p.priority_games.retain(|&g| g != app_id);
        p.skipped_games.retain(|&g| g != app_id);
        match tier {
            Tier::Priority(rank) => {
                let at = rank.saturating_sub(1).min(p.priority_games.len());
                p.priority_games.insert(at, app_id);
            }
            Tier::Indifferent => {}
            Tier::Skip => p.skipped_games.push(app_id),
        }
        save(&*repo, p)
    })
}

pub fn set_only_priority(repo: Arc<dyn PreferencesRepository>) -> SetOnlyPriority {
    Arc::new(move |only| {
        let mut p = repo.preferences();
        p.only_priority = only;
        save(&*repo, p)
    })
}

pub fn set_appear_online(repo: Arc<dyn PreferencesRepository>) -> SetAppearOnline {
    Arc::new(move |online| {
        let mut p = repo.preferences();
        p.appear_online = online;
        save(&*repo, p)
    })
}

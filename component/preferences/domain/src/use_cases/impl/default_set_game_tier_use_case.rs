use std::sync::Arc;

use steam_library::AppId;

use crate::{PreferencesError, PreferencesRepository, SetGameTierUseCase, Tier};

pub struct DefaultSetGameTierUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetGameTierUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetGameTierUseCase for DefaultSetGameTierUseCase {
    fn call(&self, app_id: AppId, tier: Tier) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
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
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

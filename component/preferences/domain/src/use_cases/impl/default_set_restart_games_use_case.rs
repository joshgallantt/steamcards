use std::sync::Arc;

use crate::{PreferencesError, PreferencesRepository, SetRestartGamesUseCase};

pub struct DefaultSetRestartGamesUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetRestartGamesUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetRestartGamesUseCase for DefaultSetRestartGamesUseCase {
    fn call(&self, restart: bool) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.restart_games = restart;
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

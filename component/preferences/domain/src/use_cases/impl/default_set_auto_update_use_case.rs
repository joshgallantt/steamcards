use std::sync::Arc;

use crate::{PreferencesError, PreferencesRepository, SetAutoUpdateUseCase};

pub struct DefaultSetAutoUpdateUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetAutoUpdateUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetAutoUpdateUseCase for DefaultSetAutoUpdateUseCase {
    fn call(&self, on: bool) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.auto_update = on;
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

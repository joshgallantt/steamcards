use std::sync::Arc;

use crate::{PreferencesError, PreferencesRepository, SetOnlyPriorityUseCase};

pub struct DefaultSetOnlyPriorityUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetOnlyPriorityUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetOnlyPriorityUseCase for DefaultSetOnlyPriorityUseCase {
    fn call(&self, only: bool) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.only_priority = only;
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

use std::sync::Arc;

use crate::{PreferencesError, PreferencesRepository, SetSkipPrivateUseCase};

pub struct DefaultSetSkipPrivateUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetSkipPrivateUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetSkipPrivateUseCase for DefaultSetSkipPrivateUseCase {
    fn call(&self, skip: bool) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.skip_private = skip;
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

use std::sync::Arc;

use crate::{PreferencesError, PreferencesRepository, SetAppearOnlineUseCase};

pub struct DefaultSetAppearOnlineUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetAppearOnlineUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetAppearOnlineUseCase for DefaultSetAppearOnlineUseCase {
    fn call(&self, online: bool) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.appear_online = online;
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

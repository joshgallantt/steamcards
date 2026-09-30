use std::sync::Arc;

use crate::{GetPreferencesUseCase, Preferences, PreferencesRepository};

pub struct DefaultGetPreferencesUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultGetPreferencesUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl GetPreferencesUseCase for DefaultGetPreferencesUseCase {
    fn call(&self) -> Preferences {
        self.repo.preferences()
    }
}

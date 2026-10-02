use std::sync::Arc;

use crate::{Preferences, PreferencesError, PreferencesRepository, SetHoursBeforeDropsUseCase};

pub struct DefaultSetHoursBeforeDropsUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetHoursBeforeDropsUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetHoursBeforeDropsUseCase for DefaultSetHoursBeforeDropsUseCase {
    fn call(&self, hours: u8) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.hours_before_drops = hours.min(Preferences::MOST_HOURS_BEFORE_DROPS);
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

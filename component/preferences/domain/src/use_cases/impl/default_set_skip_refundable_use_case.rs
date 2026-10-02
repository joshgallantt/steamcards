use std::sync::Arc;

use crate::{PreferencesError, PreferencesRepository, SetSkipRefundableUseCase};

pub struct DefaultSetSkipRefundableUseCase {
    repo: Arc<dyn PreferencesRepository>,
}

impl DefaultSetSkipRefundableUseCase {
    pub fn new(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self { repo }
    }
}

impl SetSkipRefundableUseCase for DefaultSetSkipRefundableUseCase {
    fn call(&self, skip: bool) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.skip_refundable = skip;
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}

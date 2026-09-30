use std::sync::Arc;

use crate::{Basis, PriceError, PriceRepository, SetBasisUseCase};

pub struct DefaultSetBasisUseCase {
    repo: Arc<dyn PriceRepository>,
}

impl DefaultSetBasisUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>) -> Self {
        Self { repo }
    }
}

impl SetBasisUseCase for DefaultSetBasisUseCase {
    fn call(&self, basis: Basis) -> Result<(), PriceError> {
        let mut settings = self.repo.settings();
        settings.basis = basis;
        self.repo
            .save_settings(settings)
            .map_err(|_| PriceError::Unavailable)
    }
}

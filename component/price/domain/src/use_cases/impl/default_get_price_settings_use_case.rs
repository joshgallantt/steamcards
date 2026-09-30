use std::sync::Arc;

use crate::{GetPriceSettingsUseCase, PriceRepository, PriceSettings};

pub struct DefaultGetPriceSettingsUseCase {
    repo: Arc<dyn PriceRepository>,
}

impl DefaultGetPriceSettingsUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>) -> Self {
        Self { repo }
    }
}

impl GetPriceSettingsUseCase for DefaultGetPriceSettingsUseCase {
    fn call(&self) -> PriceSettings {
        self.repo.settings()
    }
}

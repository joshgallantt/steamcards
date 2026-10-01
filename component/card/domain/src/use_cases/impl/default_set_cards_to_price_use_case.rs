use std::sync::Arc;

use game::AppId;

use crate::{CardPriceRepository, SetCardsToPriceUseCase};

pub struct DefaultSetCardsToPriceUseCase {
    repo: Arc<dyn CardPriceRepository>,
}

impl DefaultSetCardsToPriceUseCase {
    pub fn new(repo: Arc<dyn CardPriceRepository>) -> Self {
        Self { repo }
    }
}

impl SetCardsToPriceUseCase for DefaultSetCardsToPriceUseCase {
    fn call(&self, app_ids: Vec<AppId>) {
        self.repo.want(app_ids);
    }
}

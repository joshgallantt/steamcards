use std::sync::Arc;

use game::AppId;

use crate::{PriceRepository, SetGamesToPriceUseCase};

pub struct DefaultSetGamesToPriceUseCase {
    repo: Arc<dyn PriceRepository>,
}

impl DefaultSetGamesToPriceUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>) -> Self {
        Self { repo }
    }
}

impl SetGamesToPriceUseCase for DefaultSetGamesToPriceUseCase {
    fn call(&self, app_ids: Vec<AppId>) {
        self.repo.want(app_ids);
    }
}

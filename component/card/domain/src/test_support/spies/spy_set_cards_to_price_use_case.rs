use std::sync::Mutex;

use game::AppId;

use crate::SetCardsToPriceUseCase;

/// Keeps each list of games it's told to price, in order.
#[derive(Default)]
pub struct SpySetCardsToPriceUseCase {
    told: Mutex<Vec<Vec<AppId>>>,
}

impl SpySetCardsToPriceUseCase {
    pub fn told(&self) -> Vec<Vec<AppId>> {
        self.told.lock().unwrap().clone()
    }
}

impl SetCardsToPriceUseCase for SpySetCardsToPriceUseCase {
    fn call(&self, app_ids: Vec<AppId>) {
        self.told.lock().unwrap().push(app_ids);
    }
}

use std::sync::Mutex;

use game::AppId;

use crate::SetGamesToPriceUseCase;

/// Keeps each list of games it's told to price, in order.
#[derive(Default)]
pub struct SpySetGamesToPriceUseCase {
    told: Mutex<Vec<Vec<AppId>>>,
}

impl SpySetGamesToPriceUseCase {
    pub fn told(&self) -> Vec<Vec<AppId>> {
        self.told.lock().unwrap().clone()
    }
}

impl SetGamesToPriceUseCase for SpySetGamesToPriceUseCase {
    fn call(&self, app_ids: Vec<AppId>) {
        self.told.lock().unwrap().push(app_ids);
    }
}

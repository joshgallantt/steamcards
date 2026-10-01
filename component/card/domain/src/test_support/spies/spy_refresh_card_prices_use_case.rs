use std::sync::Mutex;

use game::AppId;
use tokio::task::JoinHandle;

use crate::{PriceError, RefreshCardPricesUseCase};

/// Prices nothing, keeping which games it was asked to price again.
#[derive(Default)]
pub struct SpyRefreshCardPricesUseCase {
    asked: Mutex<Vec<AppId>>,
}

impl SpyRefreshCardPricesUseCase {
    pub fn asked(&self) -> Vec<AppId> {
        self.asked.lock().unwrap().clone()
    }
}

impl RefreshCardPricesUseCase for SpyRefreshCardPricesUseCase {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<(), PriceError>> {
        self.asked.lock().unwrap().push(app_id);
        tokio::spawn(async { Ok(()) })
    }
}

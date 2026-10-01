use std::sync::Mutex;

use game::AppId;
use tokio::task::JoinHandle;

use crate::{PriceError, RefreshPricesUseCase};

/// Prices nothing, keeping which games it was asked to price again.
#[derive(Default)]
pub struct SpyRefreshPricesUseCase {
    asked: Mutex<Vec<AppId>>,
}

impl SpyRefreshPricesUseCase {
    pub fn asked(&self) -> Vec<AppId> {
        self.asked.lock().unwrap().clone()
    }
}

impl RefreshPricesUseCase for SpyRefreshPricesUseCase {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<(), PriceError>> {
        self.asked.lock().unwrap().push(app_id);
        tokio::spawn(async { Ok(()) })
    }
}

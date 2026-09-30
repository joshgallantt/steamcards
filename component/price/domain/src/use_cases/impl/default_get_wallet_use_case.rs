use std::sync::Arc;

use crate::{GetWalletUseCase, PriceRepository, Wallet};

pub struct DefaultGetWalletUseCase {
    repo: Arc<dyn PriceRepository>,
}

impl DefaultGetWalletUseCase {
    pub fn new(repo: Arc<dyn PriceRepository>) -> Self {
        Self { repo }
    }
}

impl GetWalletUseCase for DefaultGetWalletUseCase {
    fn call(&self) -> Option<Wallet> {
        self.repo.wallet()
    }
}

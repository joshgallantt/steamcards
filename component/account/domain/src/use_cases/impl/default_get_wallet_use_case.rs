use std::sync::Arc;

use crate::{AccountRepository, GetWalletUseCase, Wallet};

pub struct DefaultGetWalletUseCase {
    repo: Arc<dyn AccountRepository>,
}

impl DefaultGetWalletUseCase {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl GetWalletUseCase for DefaultGetWalletUseCase {
    fn call(&self) -> Option<Wallet> {
        self.repo.wallet()
    }
}

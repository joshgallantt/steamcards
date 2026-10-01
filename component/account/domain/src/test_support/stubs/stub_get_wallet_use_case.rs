use crate::{GetWalletUseCase, Wallet};

/// Answers with the same wallet, or with none, as if Steam hadn't said.
pub struct StubGetWalletUseCase {
    wallet: Option<Wallet>,
}

impl StubGetWalletUseCase {
    pub fn new(wallet: Option<Wallet>) -> Self {
        Self { wallet }
    }
}

impl GetWalletUseCase for StubGetWalletUseCase {
    fn call(&self) -> Option<Wallet> {
        self.wallet
    }
}

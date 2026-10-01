//! Someone signing in and out, through the account's use cases over a fake
//! repository: the account's rules, with nothing else in the way.

use std::sync::Arc;

use account::{
    Account, CheckSignInUseCase, DefaultCheckSignInUseCase, DefaultGetAccountUseCase,
    DefaultSignInUseCase, DefaultSignOutUseCase, GetAccountUseCase, LoginChallenge, SignInError,
    SignInUseCase, SignOutUseCase, test_support::FakeAccountRepository,
};
use tokio::sync::mpsc;

pub(crate) struct Player {
    pub(crate) steam: Arc<FakeAccountRepository>,
    get_account: Arc<dyn GetAccountUseCase>,
    pub(crate) check_sign_in: Arc<dyn CheckSignInUseCase>,
    sign_in: Arc<dyn SignInUseCase>,
    pub(crate) sign_out: Arc<dyn SignOutUseCase>,
}

impl Player {
    pub(crate) fn with(steam: FakeAccountRepository) -> Self {
        let steam = Arc::new(steam);
        Self {
            get_account: Arc::new(DefaultGetAccountUseCase::new(steam.clone())),
            check_sign_in: Arc::new(DefaultCheckSignInUseCase::new(steam.clone())),
            sign_in: Arc::new(DefaultSignInUseCase::new(steam.clone())),
            sign_out: Arc::new(DefaultSignOutUseCase::new(steam.clone())),
            steam,
        }
    }

    pub(crate) fn sees(&self) -> Option<Account> {
        self.get_account.call()
    }

    pub(crate) async fn signs_in(&self) -> (Vec<LoginChallenge>, Result<(), SignInError>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let outcome = self.sign_in.call(tx).await.unwrap();
        let mut seen = Vec::new();
        while let Ok(c) = rx.try_recv() {
            seen.push(c);
        }
        (seen, outcome)
    }
}

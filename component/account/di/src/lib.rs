//! Where the account domain meets its data layer: one repository over the
//! Steam session, handed to every use case. The composition root names the
//! Steam client.

use std::sync::Arc;

use account::{
    AccountRepository, CheckSignInUseCase, DefaultCheckSignInUseCase, DefaultGetAccountUseCase,
    DefaultSignInUseCase, DefaultSignOutUseCase, GetAccountUseCase, SignInUseCase, SignOutUseCase,
};
use account_data::SteamAccountRepository;
use steam_api::SteamClient;

pub struct AccountComponent {
    pub get_account: Arc<dyn GetAccountUseCase>,
    pub check_sign_in: Arc<dyn CheckSignInUseCase>,
    pub sign_in: Arc<dyn SignInUseCase>,
    pub sign_out: Arc<dyn SignOutUseCase>,
}

impl AccountComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(Arc::new(SteamAccountRepository::new(steam)))
    }

    pub fn over(repo: Arc<dyn AccountRepository>) -> Self {
        Self {
            get_account: Arc::new(DefaultGetAccountUseCase::new(repo.clone())),
            check_sign_in: Arc::new(DefaultCheckSignInUseCase::new(repo.clone())),
            sign_in: Arc::new(DefaultSignInUseCase::new(repo.clone())),
            sign_out: Arc::new(DefaultSignOutUseCase::new(repo)),
        }
    }
}

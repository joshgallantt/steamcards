//! Where the account domain meets its data layer: the sign-in the Steam
//! client holds, through one repository handed to every use case. The
//! composition root names the Steam client.

use std::sync::Arc;

use account::{
    AccountRepository, CheckSignInUseCase, DefaultCheckSignInUseCase, DefaultGetAccountUseCase,
    DefaultGetWalletUseCase, DefaultSignInUseCase, DefaultSignOutUseCase, GetAccountUseCase,
    GetWalletUseCase, SignInUseCase, SignOutUseCase,
};
use account_data::{AccountClient, DefaultAccountRepository, SteamAccountClient};
use steam_api::SteamClient;

pub struct AccountComponent {
    pub get_account: Arc<dyn GetAccountUseCase>,
    pub check_sign_in: Arc<dyn CheckSignInUseCase>,
    pub sign_in: Arc<dyn SignInUseCase>,
    pub sign_out: Arc<dyn SignOutUseCase>,
    pub get_wallet: Arc<dyn GetWalletUseCase>,
}

impl AccountComponent {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self::over(Arc::new(SteamAccountClient::new(steam)))
    }

    /// Over a client of its own: the repository is built here, and never let
    /// out.
    pub fn over(client: Arc<dyn AccountClient>) -> Self {
        let repo: Arc<dyn AccountRepository> = Arc::new(DefaultAccountRepository::new(client));
        Self {
            get_account: Arc::new(DefaultGetAccountUseCase::new(repo.clone())),
            check_sign_in: Arc::new(DefaultCheckSignInUseCase::new(repo.clone())),
            sign_in: Arc::new(DefaultSignInUseCase::new(repo.clone())),
            sign_out: Arc::new(DefaultSignOutUseCase::new(repo.clone())),
            get_wallet: Arc::new(DefaultGetWalletUseCase::new(repo)),
        }
    }
}
